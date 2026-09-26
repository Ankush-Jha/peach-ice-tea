//! The structured handoff note (R-CTX-6, T3.10).
//!
//! A summary paraphrases; some facts must survive compaction exactly: what is
//! left to do, what the user insisted on, what has already been changed, and
//! what last failed. This note is built deterministically from the
//! conversation, not by a model, and placed at the top of the S3 summary.
//! Behind `PEACH_HARNESS_HANDOFF_NOTE=1` (default off) until an A/B supports
//! it (principle 6, D-063).

use peach_domain::{Context, ContextMessage, Role, Todo, TodoStatus};

/// Environment variable that turns the note on.
pub const ENV_VAR: &str = "PEACH_HARNESS_HANDOFF_NOTE";

/// Words that mark a user message as a constraint to keep verbatim.
const CONSTRAINT_MARKERS: &[&str] =
    &["must", "never", "always", "don't", "do not", "only", "should not", "shouldn't", "required"];

/// Longest verbatim excerpt kept per constraint or failure.
const MAX_EXCERPT_CHARS: usize = 600;

/// At most this many constraints, newest last.
const MAX_CONSTRAINTS: usize = 8;

/// Whether the note is enabled for this process.
pub fn enabled() -> bool {
    std::env::var(ENV_VAR).as_deref() == Ok("1")
}

/// Builds the note from the context about to be compacted, the session's
/// todos and the files changed so far. `None` when there is nothing to hand
/// off, so no empty heading is added.
///
/// # Arguments
/// * `context` - The context before compaction.
/// * `todos` - The session's todos.
/// * `changed_files` - Paths written, patched or removed so far.
pub fn handoff_note(context: &Context, todos: &[Todo], changed_files: &[String]) -> Option<String> {
    let mut sections = Vec::new();

    if !todos.is_empty() {
        let lines: Vec<String> = todos
            .iter()
            .map(|todo| format!("- [{}] {}", status_label(&todo.status), todo.content))
            .collect();
        sections.push(format!("Todo list:\n{}", lines.join("\n")));
    }

    let constraints = user_constraints(context);
    if !constraints.is_empty() {
        let lines: Vec<String> = constraints.iter().map(|c| format!("- \"{c}\"")).collect();
        sections.push(format!("User constraints (verbatim):\n{}", lines.join("\n")));
    }

    if !changed_files.is_empty() {
        let mut files = changed_files.to_vec();
        files.sort();
        sections.push(format!("Files changed so far: {}", files.join(", ")));
    }

    if let Some((command, excerpt)) = last_failing_command(context) {
        sections.push(format!("Last failing command: `{command}`\n{excerpt}"));
    }

    if sections.is_empty() {
        return None;
    }
    Some(format!(
        "HANDOFF NOTE (exact facts kept by the harness; the summary below paraphrases)\n\n{}",
        sections.join("\n\n")
    ))
}

fn status_label(status: &TodoStatus) -> &'static str {
    match status {
        TodoStatus::Pending => "pending",
        TodoStatus::InProgress => "in progress",
        TodoStatus::Completed => "done",
        _ => "other",
    }
}

fn excerpt(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= MAX_EXCERPT_CHARS {
        trimmed.to_string()
    } else {
        let head: String = trimmed.chars().take(MAX_EXCERPT_CHARS).collect();
        format!("{head}…")
    }
}

/// User messages that state a constraint, verbatim (bounded), oldest first.
/// Earlier handoff notes are skipped so notes do not nest.
fn user_constraints(context: &Context) -> Vec<String> {
    let found: Vec<String> = context
        .messages
        .iter()
        .filter_map(|entry| match &**entry {
            ContextMessage::Text(text) if text.role == Role::User && !text.droppable => Some(text.content.as_str()),
            _ => None,
        })
        .filter(|content| !content.contains("HANDOFF NOTE"))
        .filter(|content| {
            let lower = content.to_lowercase();
            CONSTRAINT_MARKERS.iter().any(|marker| contains_word(&lower, marker))
        })
        .map(excerpt)
        .collect();
    let skip = found.len().saturating_sub(MAX_CONSTRAINTS);
    found.into_iter().skip(skip).collect()
}

fn contains_word(haystack: &str, word: &str) -> bool {
    haystack.match_indices(word).any(|(index, _)| {
        let before = haystack[..index].chars().next_back();
        let after = haystack[index + word.len()..].chars().next();
        !before.is_some_and(|c| c.is_alphanumeric()) && !after.is_some_and(|c| c.is_alphanumeric())
    })
}

/// The last shell result with a non-zero `exit_code`, as its command and an
/// excerpt of its output.
fn last_failing_command(context: &Context) -> Option<(String, String)> {
    context.messages.iter().rev().find_map(|entry| {
        let ContextMessage::Tool(result) = &**entry else {
            return None;
        };
        let text: String = result.output.values.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join("\n");
        let exit_code = attribute(&text, "exit_code")?;
        if exit_code == "0" {
            return None;
        }
        let command = attribute(&text, "command")?;
        let body = text.split_once('>').map(|(_, rest)| rest).unwrap_or(&text);
        Some((command, format!("exit code {exit_code}:\n{}", excerpt(body))))
    })
}

fn attribute(text: &str, name: &str) -> Option<String> {
    let start = text.find(&format!("{name}=\""))? + name.len() + 2;
    let end = text[start..].find('"')?;
    Some(text[start..start + end].to_string())
}

#[cfg(test)]
mod tests {
    use peach_domain::{ToolName, ToolResult};
    use pretty_assertions::assert_eq;

    use super::*;

    fn failing_shell(command: &str, exit_code: u8) -> ContextMessage {
        ContextMessage::tool_result(ToolResult::new(ToolName::new("shell")).success(format!(
            "<shell_output\n  command=\"{command}\"\n  exit_code=\"{exit_code}\"\n>\nFAILED test_add - assert 3 == 4"
        )))
    }

    #[test]
    fn test_the_note_keeps_todos_constraints_files_and_the_last_failure_exactly() {
        let context = Context::default()
            .add_message(ContextMessage::user("Fix the adder. Do not touch tests/.", None))
            .add_message(ContextMessage::user("Thanks, looks good so far", None))
            .add_message(failing_shell("pytest -q", 1))
            .add_message(failing_shell("ls", 0));
        let todos = vec![
            Todo::new("Fix add").status(TodoStatus::Completed),
            Todo::new("Run the tests").status(TodoStatus::InProgress),
        ];
        let changed = vec!["src/b.py".to_string(), "src/a.py".to_string()];

        let actual = handoff_note(&context, &todos, &changed).unwrap();

        let expected = "HANDOFF NOTE (exact facts kept by the harness; the summary below paraphrases)\n\n\
Todo list:\n- [done] Fix add\n- [in progress] Run the tests\n\n\
User constraints (verbatim):\n- \"Fix the adder. Do not touch tests/.\"\n\n\
Files changed so far: src/a.py, src/b.py\n\n\
Last failing command: `pytest -q`\nexit code 1:\nFAILED test_add - assert 3 == 4";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_nothing_to_hand_off_gives_no_note() {
        let context = Context::default().add_message(ContextMessage::user("hello", None));

        let actual = handoff_note(&context, &[], &[]);

        assert_eq!(actual, None);
    }

    #[test]
    fn test_markers_match_whole_words_only() {
        let context = Context::default()
            .add_message(ContextMessage::user("Show commonly used flags", None))
            .add_message(ContextMessage::user("You must keep the API stable", None));

        let actual = user_constraints(&context);

        assert_eq!(actual, vec!["You must keep the API stable".to_string()]);
    }
}
