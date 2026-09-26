//! Recall handles for results a compaction summarised away (R-CTX-3, T3.3).
//!
//! Forge's summary keeps that a call happened (`**Execute:** cargo test`) but
//! not what it returned. Here every tool result the summary replaced is
//! written to a handle file, and the summary gains a plain list of handles
//! with the instruction to `read` them back. `read` already takes a line
//! range and `fs_search` a pattern, so the spec's `recall(handle, start_line,
//! end_line, pattern)` needs no new tool (D-066). Behind
//! `FORGE_HARNESS_RECALL_HANDLES=1`, default off, until an A/B.

use std::path::PathBuf;

use forge_domain::{ContextMessage, MessageEntry};

/// Environment variable that turns recall handles on.
pub const ENV_VAR: &str = "FORGE_HARNESS_RECALL_HANDLES";

/// Whether recall handles are enabled for this process.
pub fn enabled() -> bool {
    std::env::var(ENV_VAR).as_deref() == Ok("1")
}

/// A tool result that was summarised away and where its full text is kept.
#[derive(Debug, Clone, PartialEq)]
pub struct RecallHandle {
    /// The tool that produced the result.
    pub tool: String,
    /// The call's id, when the provider gave one.
    pub call_id: Option<String>,
    /// File holding the full result text.
    pub path: PathBuf,
    /// Lines in that file, to help the model choose a range.
    pub lines: usize,
}

/// Tool results in `before` that are gone from `after`, each written to a
/// handle file. A result that cannot be written is left out (and logged);
/// the compaction itself is unaffected (principle 5).
///
/// # Arguments
/// * `before` - The view before compaction.
/// * `after` - The view after compaction.
pub fn write_handles(before: &[MessageEntry], after: &[MessageEntry]) -> Vec<RecallHandle> {
    before
        .iter()
        .filter(|entry| !after.contains(entry))
        .filter_map(|entry| match &entry.message {
            ContextMessage::Tool(result) => Some(result),
            _ => None,
        })
        .filter_map(|result| {
            let text: String = result.output.values.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join("\n");
            if text.is_empty() {
                return None;
            }
            let path = write_file(&text)
                .inspect_err(|error| tracing::warn!(?error, "Could not write a recall handle"))
                .ok()?;
            Some(RecallHandle {
                tool: result.name.to_string(),
                call_id: result.call_id.as_ref().map(|id| id.as_str().to_string()),
                path,
                lines: text.lines().count(),
            })
        })
        .collect()
}

/// Writes `text` to a new `forge_recall_*.txt` handle file.
pub(super) fn write_file(text: &str) -> std::io::Result<PathBuf> {
    let path = tempfile::Builder::new()
        .disable_cleanup(true)
        .prefix("forge_recall_")
        .suffix(".txt")
        .tempfile()?
        .into_temp_path()
        .to_path_buf();
    std::fs::write(&path, text)?;
    Ok(path)
}

/// The section appended to the summary, naming every handle. `None` when
/// nothing was summarised away.
///
/// # Arguments
/// * `handles` - What [`write_handles`] returned.
pub fn recall_section(handles: &[RecallHandle]) -> Option<String> {
    if handles.is_empty() {
        return None;
    }
    let lines: Vec<String> = handles
        .iter()
        .map(|handle| {
            let id = handle.call_id.as_deref().map(|id| format!(" {id}")).unwrap_or_default();
            format!("- {}{id}: read {} ({} lines)", handle.tool, handle.path.display(), handle.lines)
        })
        .collect();
    Some(format!(
        "RECOVERABLE RESULTS — the summary above omits what these tool calls returned; the full text of each \
         is kept. When you need one, read it back with `read` (a line range for part of it) or search it with \
         `fs_search` instead of running the call again.\n{}",
        lines.join("\n")
    ))
}

#[cfg(test)]
mod tests {
    use forge_domain::{ToolCallId, ToolName, ToolResult};
    use pretty_assertions::assert_eq;

    use super::*;

    fn result(id: &str, text: &str) -> MessageEntry {
        ContextMessage::tool_result(ToolResult::new(ToolName::new("shell")).call_id(ToolCallId::new(id)).success(text)).into()
    }

    #[test]
    fn test_only_results_that_left_the_view_get_handles_with_their_full_text() {
        let kept = result("c2", "still here");
        let before = vec![ContextMessage::user("task", None).into(), result("c1", "line 1\nline 2\nline 3"), kept.clone()];
        let after = vec![ContextMessage::user("summary", None).into(), kept];

        let handles = write_handles(&before, &after);

        let actual: Vec<(String, Option<String>, usize, String)> = handles
            .iter()
            .map(|h| (h.tool.clone(), h.call_id.clone(), h.lines, std::fs::read_to_string(&h.path).unwrap()))
            .collect();
        handles.iter().for_each(|h| drop(std::fs::remove_file(&h.path)));
        assert_eq!(actual, vec![("shell".to_string(), Some("c1".to_string()), 3, "line 1\nline 2\nline 3".to_string())]);
    }

    #[test]
    fn test_the_section_names_each_handle_and_how_to_read_it() {
        let handles = vec![RecallHandle {
            tool: "shell".to_string(),
            call_id: Some("c1".to_string()),
            path: PathBuf::from("/tmp/forge_recall_x.txt"),
            lines: 3,
        }];

        let actual = recall_section(&handles).unwrap();

        assert!(actual.starts_with("RECOVERABLE RESULTS"));
        assert!(actual.ends_with("- shell c1: read /tmp/forge_recall_x.txt (3 lines)"));
        assert_eq!(recall_section(&[]), None);
    }
}
