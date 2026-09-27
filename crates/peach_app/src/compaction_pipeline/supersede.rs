//! S0 supersede (R-CTX-2, T3.5): drop what later actions already replaced.
//!
//! A `read` of a file that was later read again or edited shows a state that
//! no longer exists; the output of a shell command that was later run again
//! is superseded by the newer run. Such results become stubs naming what
//! superseded them, with the full text one `read` away, so the step is
//! reversible and the model is told plainly (principle 4). Deterministic: no
//! model. The newest `retention_window` messages are never touched. "Search
//! superseded by a narrower search" (R-CTX-2) is not built: deciding what is
//! narrower needs a judgement this rule set cannot make safely (D-075).
//! Behind `PEACH_HARNESS_SUPERSEDE=1`, default off until an A/B.

use std::collections::HashMap;

use peach_domain::{Context, ContextMessage, OFFLOAD_STUB_MARKER, ToolOutput, ToolResult};

use super::recall::{RecallHandle, write_file};

/// Environment variable that turns S0 on.
pub const ENV_VAR: &str = "PEACH_HARNESS_SUPERSEDE";

const EDIT_TOOLS: &[&str] = &["write", "patch", "multi_patch", "remove", "undo"];

/// Whether S0 is enabled for this process.
pub fn enabled() -> bool {
    std::env::var(ENV_VAR).as_deref() == Ok("1")
}

/// What a call touched, for deciding what supersedes what.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Target {
    File(String),
    Command(String),
}

/// One tool call in order: its id, tool name, target, and the position of
/// the result that answered it.
struct CallInfo {
    call_id: String,
    tool: String,
    target: Option<Target>,
    result_index: Option<usize>,
}

/// Every tool call in order, each paired with the result that followed it.
/// Pairing is by occurrence, not by id alone: some providers reuse ids across
/// turns, and keying by id would then stub the newest result along with the
/// stale one (found by the end-to-end test, D-075).
fn calls_in_order(context: &Context) -> Vec<CallInfo> {
    let mut calls: Vec<CallInfo> = Vec::new();
    for (index, entry) in context.messages.iter().enumerate() {
        match &entry.message {
            ContextMessage::Text(text) => {
                for call in text.tool_calls.iter().flatten() {
                    let Some(call_id) = call.call_id.as_ref().map(|id| id.as_str().to_string())
                    else {
                        continue;
                    };
                    let tool = call.name.to_string();
                    let args = call.arguments.parse().ok();
                    let field = |name: &str| {
                        args.as_ref()
                            .and_then(|a| a.get(name))
                            .and_then(|v| v.as_str())
                            .map(str::to_string)
                    };
                    let target = match tool.as_str() {
                        "read" => field("file_path")
                            .or_else(|| field("path"))
                            .map(Target::File),
                        t if EDIT_TOOLS.contains(&t) => field("file_path")
                            .or_else(|| field("path"))
                            .map(Target::File),
                        "shell" => field("command").map(Target::Command),
                        _ => None,
                    };
                    calls.push(CallInfo { call_id, tool, target, result_index: None });
                }
            }
            ContextMessage::Tool(result) => {
                let Some(id) = result.call_id.as_ref().map(|id| id.as_str()) else {
                    continue;
                };
                // The most recent unanswered call with this id.
                if let Some(call) = calls
                    .iter_mut()
                    .rev()
                    .find(|c| c.call_id == id && c.result_index.is_none())
                {
                    call.result_index = Some(index);
                }
            }
            _ => {}
        }
    }
    calls
}

/// For each result position that is stale, why: the later call that
/// superseded it.
fn superseded(calls: &[CallInfo]) -> HashMap<usize, String> {
    let mut out = HashMap::new();
    for (i, call) in calls.iter().enumerate() {
        if !matches!(call.tool.as_str(), "read" | "shell") {
            continue;
        }
        let (Some(target), Some(result_index)) = (&call.target, call.result_index) else {
            continue;
        };
        let later = calls
            .get(i + 1..)
            .unwrap_or_default()
            .iter()
            .find(|next| next.target.as_ref() == Some(target));
        if let Some(next) = later {
            let what = match (call.tool.as_str(), next.tool.as_str(), target) {
                ("read", "read", Target::File(path)) => {
                    format!("{path} was read again later ({})", next.call_id)
                }
                ("read", edit, Target::File(path)) => {
                    format!("{path} was changed later by {edit} ({})", next.call_id)
                }
                ("shell", _, Target::Command(command)) => {
                    format!("`{command}` was run again later ({})", next.call_id)
                }
                _ => continue,
            };
            out.insert(result_index, what);
        }
    }
    out
}

/// Replaces superseded results outside the newest `retention_window`
/// messages with stubs. Returns the new context and a handle per stub; a
/// result whose handle cannot be written is left in place (fail open).
///
/// # Arguments
/// * `context` - The context to compact.
/// * `retention_window` - How many of the newest messages are never touched.
pub fn supersede(mut context: Context, retention_window: usize) -> (Context, Vec<RecallHandle>) {
    let stale = superseded(&calls_in_order(&context));
    let eligible = context.messages.len().saturating_sub(retention_window);
    let mut handles = Vec::new();
    for (index, entry) in context.messages.iter_mut().enumerate().take(eligible) {
        let ContextMessage::Tool(result) = &mut entry.message else {
            continue;
        };
        let Some(reason) = stale.get(&index) else {
            continue;
        };
        let text: String = result
            .output
            .values
            .iter()
            .filter_map(|v| v.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        if text.starts_with(OFFLOAD_STUB_MARKER) || text.is_empty() {
            continue;
        }
        let Ok(path) = write_file(&text)
            .inspect_err(|error| tracing::warn!(?error, "Could not keep a superseded result"))
        else {
            continue;
        };
        handles.push(RecallHandle {
            tool: result.name.to_string(),
            call_id: result.call_id.as_ref().map(|id| id.as_str().to_string()),
            path: path.clone(),
            lines: text.lines().count(),
        });
        let body = format!(
            "{OFFLOAD_STUB_MARKER} superseded: {reason}, so this result is out of date. \
             Full result: read {} (the complete output).]",
            path.display()
        );
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

    fn result(id: &str, tool: &str, text: &str) -> ContextMessage {
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
    fn test_reads_made_stale_by_a_reread_or_an_edit_and_reruns_are_superseded() {
        let fixture = Context::default()
            .add_message(ContextMessage::user("fix it", None))
            .add_message(call("r1", "read", r#"{"file_path":"/p/a.py"}"#))
            .add_message(result("r1", "read", "old a.py"))
            .add_message(call("r2", "read", r#"{"file_path":"/p/b.py"}"#))
            .add_message(result("r2", "read", "b.py"))
            .add_message(call("s1", "shell", r#"{"command":"pytest -q"}"#))
            .add_message(result("s1", "shell", "1 failed"))
            .add_message(call("p1", "patch", r#"{"file_path":"/p/a.py"}"#))
            .add_message(result("p1", "patch", "patched"))
            .add_message(call("s2", "shell", r#"{"command":"pytest -q"}"#))
            .add_message(result("s2", "shell", "all passed"));

        let (actual, handles) = supersede(fixture, 0);

        assert!(
            text_of(&actual, 2).contains("superseded: /p/a.py was changed later by patch (p1)")
        );
        assert_eq!(text_of(&actual, 4), "b.py", "b.py was never touched again");
        assert!(text_of(&actual, 6).contains("superseded: `pytest -q` was run again later (s2)"));
        assert_eq!(text_of(&actual, 10), "all passed", "the newest run is kept");
        assert_eq!(handles.len(), 2);
        assert_eq!(
            std::fs::read_to_string(&handles[0].path).unwrap(),
            "old a.py"
        );
        handles
            .iter()
            .for_each(|h| drop(std::fs::remove_file(&h.path)));
    }

    #[test]
    fn test_a_reused_call_id_never_stubs_the_newest_result() {
        let fixture = Context::default()
            .add_message(call("call_1", "read", r#"{"file_path":"/p/a.py"}"#))
            .add_message(result("call_1", "read", "first read"))
            .add_message(call("call_1", "read", r#"{"file_path":"/p/a.py"}"#))
            .add_message(result("call_1", "read", "second read"));

        let (actual, handles) = supersede(fixture, 0);

        assert!(text_of(&actual, 1).contains("was read again later"));
        assert_eq!(text_of(&actual, 3), "second read");
        handles
            .iter()
            .for_each(|h| drop(std::fs::remove_file(&h.path)));
    }

    #[test]
    fn test_the_retention_window_is_never_touched() {
        let fixture = Context::default()
            .add_message(call("s1", "shell", r#"{"command":"ls"}"#))
            .add_message(result("s1", "shell", "a b"))
            .add_message(call("s2", "shell", r#"{"command":"ls"}"#))
            .add_message(result("s2", "shell", "a b c"));

        let (actual, handles) = supersede(fixture.clone(), 3);

        assert_eq!(actual, fixture);
        assert!(handles.is_empty());
    }
}
