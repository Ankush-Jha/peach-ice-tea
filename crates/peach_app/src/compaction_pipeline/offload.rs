//! S1 offload (R-CTX-2, T3.6): reversible compaction before the lossy summary.
//!
//! Outside the retention window, every tool result longer than
//! `OFFLOAD_MIN_CHARS` becomes a short stub: what it was, how big, its first
//! 300 characters, and a handle to `read` it back. The call and its result
//! stay paired and every user and assistant message is untouched, so nothing
//! the model said or asked for changes; only bulk moves out of context. When
//! that is enough, S3's summary never runs. Behind `PEACH_HARNESS_OFFLOAD=1`,
//! default off until an A/B (D-074).

use peach_domain::{Context, ContextMessage, OFFLOAD_STUB_MARKER, ToolOutput, ToolResult};

use super::recall::{RecallHandle, write_file};

/// Environment variable that turns S1 on.
pub const ENV_VAR: &str = "PEACH_HARNESS_OFFLOAD";

/// Results at least this long are offloaded (R-CTX-2's `offload_min_chars`).
pub const OFFLOAD_MIN_CHARS: usize = 2_000;

/// Characters of the result kept in the stub.
const PREVIEW_CHARS: usize = 300;

/// Whether S1 is enabled for this process.
pub fn enabled() -> bool {
    std::env::var(ENV_VAR).as_deref() == Ok("1")
}

/// Offloads large tool results older than the newest `retention_window`
/// messages. Returns the new context and a handle per offloaded result. A
/// result whose handle cannot be written is left in place (fail open).
///
/// # Arguments
/// * `context` - The context to compact.
/// * `retention_window` - How many of the newest messages are never touched.
pub fn offload(mut context: Context, retention_window: usize) -> (Context, Vec<RecallHandle>) {
    let eligible = context.messages.len().saturating_sub(retention_window);
    let mut handles = Vec::new();
    for entry in context.messages.iter_mut().take(eligible) {
        let ContextMessage::Tool(result) = &mut entry.message else {
            continue;
        };
        let text: String = result
            .output
            .values
            .iter()
            .filter_map(|v| v.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        if text.chars().count() < OFFLOAD_MIN_CHARS || text.starts_with(OFFLOAD_STUB_MARKER) {
            continue;
        }
        let Ok(path) = write_file(&text)
            .inspect_err(|error| tracing::warn!(?error, "Could not offload a result"))
        else {
            continue;
        };
        handles.push(RecallHandle {
            tool: result.name.to_string(),
            call_id: result.call_id.as_ref().map(|id| id.as_str().to_string()),
            path: path.clone(),
            lines: text.lines().count(),
        });
        *result = stub(result, &text, &path);
    }
    (context, handles)
}

fn stub(result: &ToolResult, text: &str, path: &std::path::Path) -> ToolResult {
    let preview: String = text.chars().take(PREVIEW_CHARS).collect();
    let status = if result.output.is_error {
        " (an error)"
    } else {
        ""
    };
    let body = format!(
        "{OFFLOAD_STUB_MARKER} {} chars, {} lines of {} output{status}. First {PREVIEW_CHARS} chars:\n{preview}\n… \
         Full result: read {} (the complete output).]",
        text.chars().count(),
        text.lines().count(),
        result.name,
        path.display()
    );
    let mut stubbed = ToolResult::new(result.name.clone());
    stubbed.call_id = result.call_id.clone();
    stubbed.output = ToolOutput::text(body).is_error(result.output.is_error);
    stubbed
}

#[cfg(test)]
mod tests {
    use peach_domain::{ToolCallId, ToolName};
    use pretty_assertions::assert_eq;

    use super::*;

    fn result(id: &str, text: String) -> ContextMessage {
        ContextMessage::tool_result(
            ToolResult::new(ToolName::new("shell"))
                .call_id(ToolCallId::new(id))
                .success(text),
        )
    }

    #[test]
    fn test_only_large_results_outside_the_window_become_stubs_with_a_readable_handle() {
        let big = "line of build output\n".repeat(200);
        let fixture = Context::default()
            .add_message(ContextMessage::user("task", None))
            .add_message(result("old-big", big.clone()))
            .add_message(result("old-small", "ok".to_string()))
            .add_message(result("new-big", big.clone()));

        let (actual, handles) = offload(fixture.clone(), 1);

        let text = |i: usize| match &actual.messages[i].message {
            ContextMessage::Tool(r) => r.output.values[0].as_str().unwrap().to_string(),
            _ => String::new(),
        };
        assert!(
            text(1).starts_with(OFFLOAD_STUB_MARKER),
            "the old large result was not offloaded"
        );
        assert!(text(1).contains(&format!(
            "read {} (the complete output)",
            handles[0].path.display()
        )));
        assert_eq!(text(2), "ok");
        assert_eq!(
            text(3),
            big,
            "a result inside the retention window was touched"
        );
        assert_eq!(actual.messages[0], fixture.messages[0], "user text changed");
        assert_eq!(handles.len(), 1);
        assert_eq!(std::fs::read_to_string(&handles[0].path).unwrap(), big);
        let _ = std::fs::remove_file(&handles[0].path);
    }

    #[test]
    fn test_offloading_twice_changes_nothing_the_second_time() {
        let fixture = Context::default()
            .add_message(result("a", "x".repeat(5_000)))
            .add_message(ContextMessage::user("next", None));
        let (once, handles) = offload(fixture, 1);

        let (twice, again) = offload(once.clone(), 1);

        assert_eq!(twice, once);
        assert!(again.is_empty());
        handles
            .iter()
            .for_each(|h| drop(std::fs::remove_file(&h.path)));
    }
}
