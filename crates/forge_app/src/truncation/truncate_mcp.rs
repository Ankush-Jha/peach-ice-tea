// Not called anywhere in this crate yet: MCP tools are dispatched in
// `tool_registry.rs`, which this piece (W1-C) does not own. This module is
// the hand-off for T1.1's MCP branch (see `shape_mcp_output`'s doc comment
// for the exact call site) and is fully tested on its own; remove this once
// that call site lands.
#![allow(dead_code)]

use std::path::PathBuf;

use forge_config::ForgeConfig;
use forge_domain::{ToolOutput, ToolValue};

use super::notice::recovery_notice;
use super::truncate_shell::truncate_shell_output;

/// Shapes MCP tool output the same way every other tool's output is shaped
/// (R-OUT-4): head/tail line clipping plus per-line character clipping, with
/// a loud, plain-text recovery sentence whenever anything is withheld.
///
/// MCP tool output is dispatched in `tool_registry.rs`, which this crate
/// (`forge_app`) does not own here, so this function is not wired in yet.
/// The insertion point is the MCP branch of `ToolRegistry::call_inner`,
/// right after the MCP executor returns and before the output is used to
/// build the chat message:
///
/// ```ignore
/// let output = crate::truncation::shape_mcp_output(output, &config);
/// ```
///
/// Reuses the shell output caps (`max_stdout_prefix_lines`,
/// `max_stdout_suffix_lines`, `max_stdout_line_chars`) rather than adding a
/// new config field: MCP tool output is free-form text with the same shape
/// shell output has, and every other cap in `ForgeConfig` is specific to a
/// different tool's output format.
pub fn shape_mcp_output(output: ToolOutput, config: &ForgeConfig) -> ToolOutput {
    let is_error = output.is_error;
    let values = output
        .values
        .into_iter()
        .map(|value| shape_value(value, config))
        .collect();

    ToolOutput { is_error, values }
}

fn shape_value(value: ToolValue, config: &ForgeConfig) -> ToolValue {
    match value {
        ToolValue::Text(text) => ToolValue::Text(shape_text(&text, config)),
        ToolValue::AI { value, conversation_id } => {
            ToolValue::AI { value: shape_text(&value, config), conversation_id }
        }
        // Images and empty values carry nothing to withhold.
        other => other,
    }
}

fn shape_text(text: &str, config: &ForgeConfig) -> String {
    let shaped = truncate_shell_output(
        text,
        "",
        config.max_stdout_prefix_lines,
        config.max_stdout_suffix_lines,
        config.max_stdout_line_chars,
    );
    let stdout = shaped.stdout;

    let hidden_lines = stdout
        .tail_start_line
        .map(|start| start.saturating_sub(stdout.head_end_line + 1))
        .unwrap_or(0);

    if hidden_lines == 0 && stdout.truncated_lines_count == 0 {
        return text.to_string();
    }

    // Fail open (CLAUDE.md principle 5): if the dump can't be written, still
    // return the shaped, loud output — just without a working recovery path.
    let dump_path = dump_full_text(text);

    let mut body = stdout.head;
    if let Some(tail) = stdout.tail {
        body.push_str(&tail);
    }

    let mut notices = Vec::new();
    if hidden_lines > 0 {
        let recovery = match &dump_path {
            Some(path) => format!(
                "Full output: read {} (all {} lines).",
                path.display(),
                stdout.total_lines
            ),
            None => "The full output was not saved; ask the tool for a narrower result."
                .to_string(),
        };
        notices.push(recovery_notice(hidden_lines as u64, "lines", &recovery));
    }
    if stdout.truncated_lines_count > 0 {
        let recovery = match &dump_path {
            Some(path) => format!("Full output: read {}.", path.display()),
            None => "the full output was not saved".to_string(),
        };
        notices.push(format!(
            "{count} line(s) above were cut short at {max_chars} characters. {recovery}",
            count = stdout.truncated_lines_count,
            max_chars = config.max_stdout_line_chars,
        ));
    }

    format!("{body}\n\n{}", notices.join(" "))
}

/// Writes the untruncated text to a temp file so it can be recovered,
/// matching `tool_executor.rs::dump_operation`'s file naming. Synchronous
/// because this function has no access to the async `Services` used
/// elsewhere for temp-file writes; MCP output is bounded by the same caps as
/// shell output, so this is a small, one-shot write.
fn dump_full_text(text: &str) -> Option<PathBuf> {
    let path = tempfile::Builder::new()
        .disable_cleanup(true)
        .prefix("forge_mcp_")
        .suffix(".txt")
        .tempfile()
        .ok()?
        .into_temp_path()
        .to_path_buf();

    std::fs::write(&path, text).ok()?;
    Some(path)
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    fn fixture_config() -> ForgeConfig {
        ForgeConfig {
            max_stdout_prefix_lines: 2,
            max_stdout_suffix_lines: 2,
            max_stdout_line_chars: 20,
            ..ForgeConfig::default()
        }
    }

    #[test]
    fn test_short_text_passes_through_unchanged() {
        let fixture = ToolOutput::text("a short mcp result");
        let config = fixture_config();

        let actual = shape_mcp_output(fixture, &config);

        assert_eq!(actual.values[0].as_str().unwrap(), "a short mcp result");
    }

    #[test]
    fn test_long_text_is_truncated_with_a_loud_recovery_sentence() {
        let lines: Vec<String> = (1..=20).map(|i| format!("line {i}")).collect();
        let fixture = ToolOutput::text(lines.join("\n"));
        let config = fixture_config();

        let actual = shape_mcp_output(fixture, &config);
        let text = actual.values[0].as_str().unwrap();

        assert!(text.contains("line 1\n"));
        assert!(text.contains("line 20"));
        assert!(!text.contains("line 10\n"));
        assert!(text.contains("more lines not shown"));
        assert!(text.contains("Full output: read"));
    }

    #[test]
    fn test_long_single_line_is_clipped_with_a_loud_recovery_sentence() {
        let fixture = ToolOutput::text("x".repeat(100));
        let config = fixture_config();

        let actual = shape_mcp_output(fixture, &config);
        let text = actual.values[0].as_str().unwrap();

        assert!(text.contains("were cut short at 20 characters"));
        assert!(text.contains("Full output: read"));
    }

    #[test]
    fn test_error_flag_is_preserved() {
        let fixture = ToolOutput::text("boom").is_error(true);
        let config = fixture_config();

        let actual = shape_mcp_output(fixture, &config);

        assert!(actual.is_error);
    }

    #[test]
    fn test_image_values_pass_through_untouched() {
        let image = forge_domain::Image::new_base64("abc".to_string(), "image/png");
        let fixture = ToolOutput::image(image.clone());
        let config = fixture_config();

        let actual = shape_mcp_output(fixture, &config);

        assert_eq!(actual, ToolOutput::image(image));
    }
}
