use forge_domain::Transformer;

use crate::dto::anthropic::Request;

/// Transformer that keeps Anthropic prompt-cache markers stable:
/// - Always caches every system message so the static system prefix remains
///   reusable
/// - Falls back to caching the first conversation message when there is no
///   system prompt so single-turn requests still establish a reusable prefix
/// - Uses exactly one rolling message-level marker on the newest message
pub struct SetCache;

impl Transformer for SetCache {
    type Value = Request;

    /// Applies the default Anthropic cache strategy:
    /// 1. Cache every system message when present, otherwise cache the first
    ///    conversation message.
    /// 2. Cache only the last message as the rolling message-level marker.
    fn transform(&mut self, mut request: Self::Value) -> Self::Value {
        let len = request.get_messages().len();
        let sys_len = request.system.as_ref().map_or(0, |msgs| msgs.len());

        if len == 0 && sys_len == 0 {
            return request;
        }

        let has_system_prompt = request
            .system
            .as_ref()
            .is_some_and(|messages| !messages.is_empty());

        if let Some(system_messages) = request.system.as_mut() {
            for message in system_messages.iter_mut() {
                *message = std::mem::take(message).cached(true);
            }
        }

        for message in request.get_messages_mut().iter_mut() {
            *message = std::mem::take(message).cached(false);
        }

        if !has_system_prompt
            && len > 0
            && let Some(first_message) = request.get_messages_mut().first_mut()
        {
            *first_message = std::mem::take(first_message).cached(true);
        }

        if let Some(message) = request.get_messages_mut().last_mut() {
            *message = std::mem::take(message).cached(true);
        }

        // harness: D-084 — a breakpoint on the last tool definition caches the
        // whole (static, ~38 KB) tool array on its own, instead of only as part
        // of the system prefix. Anthropic rejects more than
        // `MAX_CACHE_BREAKPOINTS` per request, so it is added only when it fits.
        for tool in request.tools.iter_mut() {
            tool.set_cached(false);
        }
        if count_cache_breakpoints(&request) < MAX_CACHE_BREAKPOINTS
            && let Some(last_tool) = request.tools.last_mut()
        {
            last_tool.set_cached(true);
        }

        request
    }
}

/// Anthropic's limit on cache breakpoints in one request (a 400 beyond it).
pub const MAX_CACHE_BREAKPOINTS: usize = 4;

/// Cache breakpoints set anywhere in `request`: system messages, message
/// content and tool definitions.
pub fn count_cache_breakpoints(request: &Request) -> usize {
    let system = request.system.as_ref().map_or(0, |messages| messages.iter().filter(|m| m.is_cached()).count());
    let messages = request
        .get_messages()
        .iter()
        .map(|message| message.content.iter().filter(|content| content.is_cached()).count())
        .sum::<usize>();
    let tools = request.tools.iter().filter(|tool| tool.is_cached()).count();
    system + messages + tools
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use forge_domain::{Context, ContextMessage, ModelId, Role, TextMessage};
    use pretty_assertions::assert_eq;

    use super::*;

    fn create_test_context_with_system(
        system_messages: &str,
        conversation_messages: &str,
    ) -> String {
        let mut messages = Vec::new();

        // Add system messages to the regular messages array for Anthropic
        // format
        for c in system_messages.chars() {
            match c {
                's' => messages.push(
                    ContextMessage::Text(TextMessage::new(Role::System, c.to_string())).into(),
                ),
                _ => panic!("Invalid character in system message: {}", c),
            }
        }

        // Add conversation messages
        for c in conversation_messages.chars() {
            match c {
                'u' => messages.push(
                    ContextMessage::Text(
                        TextMessage::new(Role::User, c.to_string())
                            .model(ModelId::new("claude-3-5-sonnet-20241022")),
                    )
                    .into(),
                ),
                'a' => messages.push(
                    ContextMessage::Text(TextMessage::new(Role::Assistant, c.to_string())).into(),
                ),
                _ => panic!("Invalid character in conversation message: {}", c),
            }
        }

        let context = Context {
            conversation_id: None,
            messages,
            tools: vec![],
            tool_choice: None,
            max_tokens: None,
            temperature: None,
            top_p: None,
            top_k: None,
            reasoning: None,
            stream: None,
            response_format: None,
            initiator: None,
        };

        let request = Request::try_from(context).expect("Failed to convert context to request");
        let mut transformer = SetCache;
        let request = transformer.transform(request);

        let mut output = String::new();

        // Check if first system message is cached
        let system_cached = request
            .system
            .as_ref()
            .and_then(|sys| sys.first())
            .map(|msg| msg.is_cached())
            .unwrap_or(false);

        if system_cached {
            output.push('[');
        }
        output.push_str(system_messages);

        // Check which regular messages are cached
        let cached_indices = request
            .get_messages()
            .iter()
            .enumerate()
            .filter(|(_, m)| m.is_cached())
            .map(|(i, _)| i)
            .collect::<HashSet<usize>>();

        for (i, c) in conversation_messages.chars().enumerate() {
            if cached_indices.contains(&i) {
                output.push('[');
            }
            output.push(c);
        }

        output
    }

    /// A transformed request with `system` system messages, the conversation
    /// `turns` ('u'/'a'), and `tools` tool definitions (D-084).
    fn transformed_with_tools(system: usize, turns: &str, tools: usize) -> Request {
        let mut messages: Vec<forge_domain::MessageEntry> = (0..system)
            .map(|i| ContextMessage::Text(TextMessage::new(Role::System, format!("s{i}"))).into())
            .collect();
        for c in turns.chars() {
            let role = if c == 'u' { Role::User } else { Role::Assistant };
            messages.push(ContextMessage::Text(TextMessage::new(role, c.to_string())).into());
        }
        let context = Context::default()
            .messages(messages)
            .tools((0..tools).map(|i| forge_domain::ToolDefinition::new(format!("tool_{i}"))).collect::<Vec<_>>());
        SetCache.transform(Request::try_from(context).expect("Failed to convert context to request"))
    }

    #[test]
    fn test_only_the_last_tool_gets_a_breakpoint() {
        let request = transformed_with_tools(1, "uau", 5);

        let actual: Vec<bool> = request.tools.iter().map(|tool| tool.is_cached()).collect();

        assert_eq!(actual, vec![false, false, false, false, true]);
    }

    #[test]
    fn test_breakpoints_never_exceed_anthropics_limit_for_the_shapes_forge_sends() {
        // Forge sends 2 system messages (prompt + system information), 3 under
        // OAuth; 0 and 1 also occur. Short and long conversations each.
        for system in 0..=3 {
            for turns in ["u", "uau", "uauauauaua"] {
                let request = transformed_with_tools(system, turns, 12);

                let actual = count_cache_breakpoints(&request);

                assert!(actual <= MAX_CACHE_BREAKPOINTS, "{system} system + {turns}: {actual} breakpoints");
            }
        }
    }

    #[test]
    fn test_the_tool_breakpoint_never_pushes_a_request_over_the_limit() {
        // With 4 system messages upstream's own markers already reach 5 (every
        // system message + the last message; recorded in D-084). The tool
        // marker must not add to that.
        let request = transformed_with_tools(4, "u", 12);

        let actual = request.tools.iter().any(|tool| tool.is_cached());

        assert!(!actual);
    }

    #[test]
    fn test_the_tool_breakpoint_is_dropped_when_the_limit_is_already_reached() {
        let request = transformed_with_tools(3, "uau", 4);

        let actual = (count_cache_breakpoints(&request), request.tools.iter().any(|tool| tool.is_cached()));

        assert_eq!(actual, (4, false));
    }

    fn create_test_context(message: impl ToString) -> String {
        create_test_context_with_system("", &message.to_string())
    }

    #[test]
    fn test_single_message() {
        let actual = create_test_context("u");
        let expected = "[u";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_two_messages() {
        let actual = create_test_context("ua");
        let expected = "[u[a";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_three_messages_cache_first_and_last_only() {
        let actual = create_test_context("uau");
        let expected = "[ua[u";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_four_messages_cache_first_and_last_only() {
        let actual = create_test_context("uaua");
        let expected = "[uau[a";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_five_messages_cache_first_and_last_only() {
        let actual = create_test_context("uauau");
        let expected = "[uaua[u";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_longer_conversation_caches_first_and_last_only() {
        let actual = create_test_context("uauauauaua");
        let expected = "[uauauauau[a";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_with_system_message_single_conversation_message() {
        let actual = create_test_context_with_system("s", "u");
        let expected = "[s[u";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_with_system_message_multiple_conversation_messages() {
        let actual = create_test_context_with_system("ss", "uaua");
        let expected = "[ssuau[a";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_with_system_message_long_conversation() {
        let actual = create_test_context_with_system("s", "uauauauaua");
        let expected = "[suauauauau[a";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_only_system_message() {
        let actual = create_test_context_with_system("s", "");
        let expected = "[s";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_multiple_system_messages_all_cached() {
        let fixture = Context {
            conversation_id: None,
            messages: vec![
                ContextMessage::Text(TextMessage::new(Role::System, "first")).into(),
                ContextMessage::Text(TextMessage::new(Role::System, "second")).into(),
                ContextMessage::Text(
                    TextMessage::new(Role::User, "user")
                        .model(ModelId::new("claude-3-5-sonnet-20241022")),
                )
                .into(),
            ],
            tools: vec![],
            tool_choice: None,
            max_tokens: None,
            temperature: None,
            top_p: None,
            top_k: None,
            reasoning: None,
            stream: None,
            response_format: None,
            initiator: None,
        };

        let request = Request::try_from(fixture).expect("Failed to convert context to request");
        let mut transformer = SetCache;
        let request = transformer.transform(request);

        let expected = vec![true, true];
        let actual = request
            .system
            .as_ref()
            .unwrap()
            .iter()
            .map(|message| message.is_cached())
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
        assert!(request.get_messages()[0].is_cached());
    }
}
