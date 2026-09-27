//! S2 score (R-CTX-2/R-CTX-4, T3.9 in part): relevance-scored, reversible cuts.
//!
//! Every tool result outside the pinned window is summarised for the
//! relevance scorer (tool, redacted input preview, status, size, position,
//! and whether later text mentions its paths), and the scorer's decision is
//! applied: keep it, cut it to its head, or reduce it to a stub. Both cuts
//! keep the full text one `read` away, and the call stays paired with its
//! result. The scorer is T3.8's `HeuristicScorer`, which needs no model: the
//! spec's `LlmScorer` would spend a model request on every compaction, which
//! the free-tier stance (D-069) cannot afford, so it stays deferred (D-077).
//! Behind `PEACH_HARNESS_SCORE_STAGE=1`, default off until an A/B.

use peach_domain::{Context, ContextMessage, OFFLOAD_STUB_MARKER, Role, ToolOutput, ToolResult};
use peach_harness::scorer::heuristic::HeuristicScorer;
use peach_harness::scorer::plan::{
    Decision, ResultStatus, ScorerConfig, ToolCallSummary, is_referenced_later,
};

use super::recall::{RecallHandle, write_file};

/// Environment variable that turns S2 on.
pub const ENV_VAR: &str = "PEACH_HARNESS_SCORE_STAGE";

/// Whether S2 is enabled for this process.
pub fn enabled() -> bool {
    std::env::var(ENV_VAR).as_deref() == Ok("1")
}

const PREVIEW_CHARS: usize = 200;

/// Text of a tool result, joined.
fn result_text(result: &ToolResult) -> String {
    result
        .output
        .values
        .iter()
        .filter_map(|v| v.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Summaries of every scorable result, with a synthetic id per position:
/// provider call ids can repeat (D-075), positions cannot.
fn summaries(context: &Context) -> Vec<ToolCallSummary> {
    let mut summaries = Vec::new();
    for (index, entry) in context.messages.iter().enumerate() {
        let ContextMessage::Tool(result) = &entry.message else {
            continue;
        };
        let text = result_text(result);
        if text.starts_with(OFFLOAD_STUB_MARKER) {
            continue;
        }
        // The most recent call before this result with its id, for the input.
        let input = result.call_id.as_ref().and_then(|id| {
            context
                .messages
                .get(..index)
                .unwrap_or_default()
                .iter()
                .rev()
                .find_map(|earlier| match &earlier.message {
                    ContextMessage::Text(text) => text
                        .tool_calls
                        .iter()
                        .flatten()
                        .find(|call| call.call_id.as_ref() == Some(id))
                        .map(|call| call.arguments.clone().into_string()),
                    _ => None,
                })
        });
        let preview: String = input
            .unwrap_or_default()
            .chars()
            .take(PREVIEW_CHARS)
            .collect();
        let later: String = context
            .messages
            .get(index + 1..)
            .unwrap_or_default()
            .iter()
            .filter_map(|later| match &later.message {
                ContextMessage::Text(text) if matches!(text.role, Role::User | Role::Assistant) => {
                    Some(text.content.as_str())
                }
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        let status = if result.output.is_error {
            ResultStatus::Error
        } else {
            ResultStatus::Ok
        };
        let referenced = is_referenced_later(&preview, &later);
        summaries.push(
            ToolCallSummary::new(
                format!("m{index}"),
                result.name.to_string(),
                preview,
                status,
                text.chars().count(),
                index,
            )
            .referenced_later(referenced),
        );
    }
    summaries
}

/// Applies relevance-scored cuts outside the newest `retention_window`
/// messages. Returns the new context and a handle per cut; a result whose
/// handle cannot be written is left in place (fail open), and a scorer
/// failure keeps everything (R-CTX-5, enforced by `build_plan`).
///
/// # Arguments
/// * `context` - The context to compact.
/// * `retention_window` - How many of the newest messages are never touched.
pub fn score(mut context: Context, retention_window: usize) -> (Context, Vec<RecallHandle>) {
    let goal = context
        .messages
        .iter()
        .find_map(|entry| match &entry.message {
            ContextMessage::Text(text) if text.role == Role::User => Some(text.content.clone()),
            _ => None,
        })
        .unwrap_or_default();
    let config = ScorerConfig {
        preserve_recent_messages: retention_window,
        ..ScorerConfig::default()
    };
    // harness: R-CTX-4 (D-098) — an external scorer (e.g. a Jev adapter)
    // when one is configured, the built-in heuristic otherwise.
    let external = peach_harness::scorer::external::ExternalScorer::from_env();
    let heuristic = HeuristicScorer::new();
    let scorer: &dyn peach_harness::scorer::RelevanceScorer = match &external {
        Some(external) => external,
        None => &heuristic,
    };
    let plan = peach_harness::scorer::build_plan(scorer, &summaries(&context), &goal, &config);

    let mut handles = Vec::new();
    for scored in plan.scored {
        let Some(index) = scored
            .call_id
            .strip_prefix('m')
            .and_then(|i| i.parse::<usize>().ok())
        else {
            continue;
        };
        let head_chars = match scored.decision {
            Decision::Keep => continue,
            Decision::Truncate { head_chars } => head_chars,
            Decision::Drop => 0,
        };
        let Some(ContextMessage::Tool(result)) = context
            .messages
            .get_mut(index)
            .map(|entry| &mut entry.message)
        else {
            continue;
        };
        let text = result_text(result);
        if text.chars().count() <= head_chars {
            continue;
        }
        let Ok(path) = write_file(&text)
            .inspect_err(|error| tracing::warn!(?error, "Could not keep a scored result"))
        else {
            continue;
        };
        handles.push(RecallHandle {
            tool: result.name.to_string(),
            call_id: result.call_id.as_ref().map(|id| id.as_str().to_string()),
            path: path.clone(),
            lines: text.lines().count(),
        });
        let head: String = text.chars().take(head_chars).collect();
        let body = if head_chars == 0 {
            format!(
                "{OFFLOAD_STUB_MARKER} scored as no longer relevant: {} chars of {} output. \
                 Full result: read {} (the complete output).]",
                text.chars().count(),
                result.name,
                path.display()
            )
        } else {
            format!(
                "{OFFLOAD_STUB_MARKER} cut to its first {head_chars} of {} chars by relevance scoring:\n{head}\n… \
                 Full result: read {} (the complete output).]",
                text.chars().count(),
                path.display()
            )
        };
        let mut stub = ToolResult::new(result.name.clone());
        stub.call_id = result.call_id.clone();
        stub.output = ToolOutput::text(body).is_error(result.output.is_error);
        *result = stub;
    }
    (context, handles)
}

#[cfg(test)]
mod tests {
    use peach_domain::{ToolCallArguments, ToolCallFull, ToolCallId, ToolName};
    use pretty_assertions::assert_eq;

    use super::*;

    fn call(id: &str, tool: &str, args: &str) -> ContextMessage {
        let call = ToolCallFull::new(ToolName::new(tool))
            .call_id(ToolCallId::new(id))
            .arguments(ToolCallArguments::from_json(args));
        ContextMessage::assistant("", None, None, Some(vec![call]))
    }

    fn result(id: &str, tool: &str, text: String) -> ContextMessage {
        ContextMessage::tool_result(
            ToolResult::new(ToolName::new(tool))
                .call_id(ToolCallId::new(id))
                .success(text),
        )
    }

    fn text_of(context: &Context, i: usize) -> String {
        match &context.messages[i].message {
            ContextMessage::Tool(r) => r.output.values[0].as_str().unwrap().to_string(),
            _ => String::new(),
        }
    }

    #[test]
    fn test_an_unreferenced_old_read_is_cut_and_a_referenced_one_is_kept() {
        let big = "x".repeat(20_000);
        let fixture = Context::default()
            .add_message(ContextMessage::user("Fix the adder.", None))
            .add_message(call("a", "read", r#"{"file_path":"/p/unused.py"}"#))
            .add_message(result("a", "read", big.clone()))
            .add_message(call("b", "read", r#"{"file_path":"/p/stats.py"}"#))
            .add_message(result("b", "read", big.clone()))
            .add_message(ContextMessage::assistant(
                "The bug is in stats.py.",
                None,
                None,
                None,
            ))
            .add_message(ContextMessage::user("go on", None))
            .add_message(ContextMessage::assistant("ok", None, None, None));

        let (actual, handles) = score(fixture, 2);

        assert!(
            text_of(&actual, 2).starts_with(OFFLOAD_STUB_MARKER),
            "the unused read was not cut"
        );
        assert_eq!(
            text_of(&actual, 4),
            big,
            "the read the conversation refers to was cut"
        );
        assert_eq!(handles.len(), 1);
        assert_eq!(std::fs::read_to_string(&handles[0].path).unwrap(), big);
        handles
            .iter()
            .for_each(|h| drop(std::fs::remove_file(&h.path)));
    }

    #[test]
    fn test_scoring_twice_changes_nothing_the_second_time() {
        let fixture = Context::default()
            .add_message(ContextMessage::user("task", None))
            .add_message(call("a", "read", r#"{"file_path":"/p/old.py"}"#))
            .add_message(result("a", "read", "y".repeat(20_000)))
            .add_message(ContextMessage::user("next", None))
            .add_message(ContextMessage::assistant("ok", None, None, None));
        let (once, handles) = score(fixture, 2);

        let (twice, again) = score(once.clone(), 2);

        assert_eq!(twice, once);
        assert!(again.is_empty());
        handles
            .iter()
            .for_each(|h| drop(std::fs::remove_file(&h.path)));
    }
}
