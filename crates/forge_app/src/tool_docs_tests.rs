//! harness: D-039 — compacting tool descriptions may remove worked examples
//! and nothing else, for every tool in the built-in catalog.

use forge_domain::ToolCatalog;
use pretty_assertions::assert_eq;
use strum::IntoEnumIterator;

/// Lines of `text` outside `<example…>` … `</example…>` blocks, found by a
/// line scan independent of the compactor's own implementation.
fn lines_outside_examples(text: &str) -> Vec<&str> {
    let mut inside = false;
    let mut kept = vec![];
    for line in text.lines() {
        let opens = line.contains("<example");
        let closes = line.contains("</example");
        if opens && !closes {
            inside = true;
        } else if closes {
            inside = false;
        } else if !inside && !opens {
            kept.push(line);
        }
    }
    kept
}

fn is_example_heading(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.contains("example") && (line.trim_start().starts_with('#') || line.trim_end().ends_with(':'))
}

#[test]
fn test_compaction_removes_only_examples_in_every_catalog_tool() {
    for tool in ToolCatalog::iter() {
        let original = tool.definition().description;
        let compacted = forge_harness::tool_docs::compact(&original);
        let name = tool.definition().name.to_string();

        // Nothing added or reworded: compacted lines appear, in order, in the original.
        let mut source = original.lines();
        for line in compacted.lines() {
            assert!(
                source.any(|candidate| candidate == line),
                "{name}: compacted line not found in order in the original: {line:?}"
            );
        }
        // Nothing but examples removed.
        let expected: Vec<&str> = lines_outside_examples(&original)
            .into_iter()
            .filter(|line| !line.trim().is_empty() && !is_example_heading(line))
            .collect();
        let actual: Vec<&str> = compacted
            .lines()
            .filter(|line| !line.trim().is_empty() && !is_example_heading(line))
            .collect();
        assert_eq!(actual, expected, "{name}");
        assert!(!compacted.contains("<example"), "{name}: an example block survived");
    }
}
