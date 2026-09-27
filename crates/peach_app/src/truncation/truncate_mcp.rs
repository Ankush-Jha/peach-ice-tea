use std::path::PathBuf;

use peach_config::PeachConfig;
use peach_domain::{ToolOutput, ToolValue};

use super::notice::recovery_notice;
use super::truncate_shell::truncate_shell_output;

/// Shapes MCP tool output the same way every other tool's output is shaped
/// (R-OUT-4): head/tail line clipping plus per-line character clipping, with
/// a loud, plain-text recovery sentence whenever anything is withheld.
///
/// Called from the MCP branch of `ToolRegistry::call_inner`, right after the
/// MCP executor returns (D-059).
///
/// Reuses the shell output caps (`max_stdout_prefix_lines`,
/// `max_stdout_suffix_lines`, `max_stdout_line_chars`) rather than adding a
/// new config field: MCP tool output is free-form text with the same shape
/// shell output has, and every other cap in `PeachConfig` is specific to a
/// different tool's output format.
///
/// Returns the shaped output and the files the full text was saved to, so the
/// caller can register them as recovery handles (R-OUT-3).
pub fn shape_mcp_output(output: ToolOutput, config: &PeachConfig) -> (ToolOutput, Vec<PathBuf>) {
    let is_error = output.is_error;
    let mut dumps = Vec::new();
    let values = output
        .values
        .into_iter()
        .map(|value| shape_value(value, config, &mut dumps))
        .collect();

    (ToolOutput { is_error, values }, dumps)
}

fn shape_value(value: ToolValue, config: &PeachConfig, dumps: &mut Vec<PathBuf>) -> ToolValue {
    match value {
        ToolValue::Text(text) => ToolValue::Text(shape_text(&text, config, dumps)),
        ToolValue::AI { value, conversation_id } => {
            ToolValue::AI { value: shape_text(&value, config, dumps), conversation_id }
        }
        // Images and empty values carry nothing to withhold.
        other => other,
    }
}

fn shape_text(text: &str, config: &PeachConfig, dumps: &mut Vec<PathBuf>) -> String {
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
    dumps.extend(dump_path.clone());

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
            None => {
                "The full output was not saved; ask the tool for a narrower result.".to_string()
            }
        };
        notices.push(recovery_notice(hidden_lines as u64, "lines", &recovery));
    }
    if stdout.truncated_lines_count > 0 {
        let recovery = match &dump_path {
            Some(path) => format!(
                "Full output: read {} (the complete output).",
                path.display()
            ),
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
        .prefix("peach_mcp_")
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

    fn fixture_config() -> PeachConfig {
        PeachConfig {
            max_stdout_prefix_lines: 2,
            max_stdout_suffix_lines: 2,
            max_stdout_line_chars: 20,
            ..PeachConfig::default()
        }
    }

    #[test]
    fn test_short_text_passes_through_unchanged() {
        let fixture = ToolOutput::text("a short mcp result");
        let config = fixture_config();

        let (actual, _) = shape_mcp_output(fixture, &config);

        assert_eq!(actual.values[0].as_str().unwrap(), "a short mcp result");
    }

    #[test]
    fn test_long_text_is_truncated_with_a_loud_recovery_sentence() {
        let lines: Vec<String> = (1..=20).map(|i| format!("line {i}")).collect();
        let fixture = ToolOutput::text(lines.join("\n"));
        let config = fixture_config();

        let (actual, _) = shape_mcp_output(fixture, &config);
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

        let (actual, _) = shape_mcp_output(fixture, &config);
        let text = actual.values[0].as_str().unwrap();

        assert!(text.contains("were cut short at 20 characters"));
        assert!(text.contains("Full output: read"));
    }

    #[test]
    fn test_the_saved_full_text_is_returned_as_a_handle() {
        let lines: Vec<String> = (1..=20).map(|i| format!("line {i}")).collect();
        let fixture = ToolOutput::text(lines.join("\n"));
        let config = fixture_config();

        let (output, dumps) = shape_mcp_output(fixture, &config);
        let text = output.values[0].as_str().unwrap();

        assert_eq!(dumps.len(), 1);
        assert!(text.contains(&dumps[0].display().to_string()));
        assert_eq!(
            std::fs::read_to_string(&dumps[0]).unwrap(),
            lines.join("\n")
        );
        let _ = std::fs::remove_file(&dumps[0]);
    }

    #[test]
    fn test_error_flag_is_preserved() {
        let fixture = ToolOutput::text("boom").is_error(true);
        let config = fixture_config();

        let (actual, _) = shape_mcp_output(fixture, &config);

        assert!(actual.is_error);
    }

    #[test]
    fn test_image_values_pass_through_untouched() {
        let image = peach_domain::Image::new_base64("abc".to_string(), "image/png");
        let fixture = ToolOutput::image(image.clone());
        let config = fixture_config();

        let (actual, _) = shape_mcp_output(fixture, &config);

        assert_eq!(actual, ToolOutput::image(image));
    }
}
