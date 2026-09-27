//! Smaller tool descriptions (fixed cost per request; D-039).
//!
//! Tool definitions are re-sent with every request, retries included, and
//! D-032 measured them at 75% of a Gemini request. About a fifth of that is
//! worked examples inside `<example>` blocks, mostly in `todo_write` and
//! `task`. [`compact`] removes those blocks from the rendered description and
//! keeps everything else: every rule, list, parameter note and dynamic
//! section (such as `task`'s agent list). Nothing is paraphrased, so no
//! instruction the model relies on can be reworded away.
//!
//! **On by default** since D-085: A/Bs on two model families (DeepSeek V4.1
//! Flash, D-039 report; Nemotron 3 Ultra, 6 fixtures × 2 seeds) kept success at
//! 100% and cut input tokens (−17.6%, −6.6%). `ENV_VAR=0` turns it off.

/// Environment variable that disables compaction when set to `0`.
pub const ENV_VAR: &str = "PEACH_HARNESS_COMPACT_TOOL_DOCS";

/// Whether compaction is enabled for this process: on unless `ENV_VAR=0`.
pub fn enabled() -> bool {
    std::env::var(ENV_VAR).map_or(true, |value| value != "0")
}

/// `description` without its `<example…>…</example…>` blocks, and without
/// headings or lead-in lines those blocks leave with nothing under them.
///
/// # Arguments
/// * `description` - A rendered tool description.
pub fn compact(description: &str) -> String {
    let mut kept = String::with_capacity(description.len());
    let mut rest = description;
    while let Some(start) = rest.find("<example") {
        let (before, after) = rest.split_at(start);
        kept.push_str(before);
        // The closing tag's name may differ from the opening one's
        // (`<example_agent_descriptions>` … `</example_agent_description>`
        // is in the upstream text), so close on the first `</example`.
        match after
            .find("</example")
            .and_then(|close| after.get(close..)?.find('>').map(|end| close + end + 1))
        {
            Some(end) => rest = after.get(end..).unwrap_or_default(),
            None => {
                // Unclosed: keep the text rather than drop the remainder.
                kept.push_str(after);
                rest = "";
            }
        }
    }
    kept.push_str(rest);
    drop_empty_sections(&kept)
}

/// Removes heading lines (`## …`) and lead-in lines ending in `:` that are
/// followed only by blank lines until the next heading or the end, then
/// collapses runs of blank lines.
fn drop_empty_sections(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let is_heading = |line: &str| line.trim_start().starts_with('#');
    let mut out: Vec<&str> = Vec::with_capacity(lines.len());
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        let lead_in = is_heading(line)
            || (trimmed.ends_with(':') && trimmed.to_ascii_lowercase().contains("example"));
        if lead_in {
            let next = lines
                .get(index + 1..)
                .unwrap_or_default()
                .iter()
                .find(|next| !next.trim().is_empty());
            if next.is_none_or(|next| is_heading(next)) {
                continue;
            }
        }
        out.push(line);
    }
    let mut result = String::with_capacity(text.len());
    let mut blank_run = 0;
    for line in out {
        if line.trim().is_empty() {
            blank_run += 1;
            if blank_run > 1 {
                continue;
            }
        } else {
            blank_run = 0;
        }
        result.push_str(line);
        result.push('\n');
    }
    result.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_examples_go_and_every_rule_stays() {
        let fixture = "Use this tool to track tasks.\n\n## Rules\n- Only one in_progress.\n\n## Examples of When to Use\n\n<example>\nUser: add dark mode\n</example>\n\n<example>\nUser: rename\n</example>\n\n## When NOT to Use\n- trivial tasks\n\nExample usage:\n\n<example_agent_descriptions>\n\"x\": y\n</example_agent_description>\n";

        let actual = compact(fixture);

        let expected = "Use this tool to track tasks.\n\n## Rules\n- Only one in_progress.\n\n## When NOT to Use\n- trivial tasks";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_text_without_examples_is_unchanged_apart_from_trailing_space() {
        let fixture = "Executes a shell command.\n\n- Quote paths with spaces.\n- Examples of proper quoting:\n  - cd \"path with spaces\"\n";

        let actual = compact(fixture);

        assert_eq!(actual, fixture.trim_end());
    }

    #[test]
    fn test_an_unclosed_example_keeps_the_rest_rather_than_dropping_it() {
        let fixture = "Rule one.\n<example>\nnever closed\nRule two.";

        let actual = compact(fixture);

        assert_eq!(actual, fixture);
    }
}
