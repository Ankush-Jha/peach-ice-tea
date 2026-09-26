//! harness: T2.1 / R-LOOP-1 — which tool calls may run concurrently.
//!
//! Only tools that cannot change the workspace or the conversation's
//! bookkeeping in an order-dependent way are read-only. Everything else —
//! writes, patches, shell (which can do anything), todo updates, unknown and
//! MCP tools — runs exclusively, in order, exactly as before.

use forge_domain::ToolName;

/// Environment variable that enables concurrent read-only batches when `1`.
pub const ENV_VAR: &str = "FORGE_HARNESS_PARALLEL_READONLY";

/// Wire names of the read-only built-in tools.
const READ_ONLY: &[&str] = &["read", "fs_search", "sem_search", "fetch", "skill", "todo_read"];

/// Whether this process enables concurrent read-only batches.
pub fn enabled() -> bool {
    std::env::var(ENV_VAR).is_ok_and(|value| value == "1")
}

/// Whether a tool call may run concurrently with its read-only neighbours.
pub fn is_read_only(name: &ToolName) -> bool {
    let name = name.as_str().trim().to_ascii_lowercase();
    READ_ONLY.contains(&name.as_str())
}

/// Splits calls into consecutive segments: each maximal run of two or more
/// read-only calls is one concurrent segment, and every other call is a
/// segment of its own. Concatenating the segments gives back the input, in
/// order.
pub fn segments<T>(calls: &[T], name: impl Fn(&T) -> &ToolName) -> Vec<(bool, std::ops::Range<usize>)> {
    let mut segments = Vec::new();
    let mut index = 0;
    while index < calls.len() {
        let mut end = index;
        while end < calls.len() && is_read_only(name(&calls[end])) {
            end += 1;
        }
        if end - index >= 2 {
            segments.push((true, index..end));
            index = end;
        } else {
            segments.push((false, index..index + 1));
            index += 1;
        }
    }
    segments
}

#[cfg(test)]
mod tests {
    use forge_domain::ToolCatalog;
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_every_read_only_name_is_a_real_catalog_tool() {
        for name in READ_ONLY {
            assert!(ToolCatalog::contains(&ToolName::new(*name)), "{name} is not a catalog tool");
        }
    }

    #[test]
    fn test_only_runs_of_two_or_more_reads_become_concurrent() {
        let fixture: Vec<ToolName> = ["read", "fs_search", "write", "read", "shell", "READ", "fetch", "todo_read"]
            .into_iter()
            .map(ToolName::new)
            .collect();

        let actual = segments(&fixture, |name| name);

        let expected = vec![(true, 0..2), (false, 2..3), (false, 3..4), (false, 4..5), (true, 5..8)];
        assert_eq!(actual, expected);
    }
}
