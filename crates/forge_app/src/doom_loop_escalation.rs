//! harness: R-LOOP-5 — graduated, enforced response to a tool called
//! repeatedly with the exact same arguments, ahead of (and orthogonal to)
//! the sequence-pattern nudge in `hooks/doom_loop.rs`. That detector only
//! ever adds a reminder after the fact; this classifies a call **before**
//! it runs and can skip execution or pause the run outright (CLAUDE.md
//! principle 3: enforce in the runtime, don't rely on prompts).
//!
//! Off unless [`ENV_VAR`] is `1` (principle 6: an A/B first). Counters are
//! process-local to one `Orchestrator::run()` call — not persisted onto
//! `Conversation` — same reasoning as `TaskMetrics::recent_shell_commands`
//! (CLAUDE.md principle 5, fail open): a conversation resumed via a fresh
//! dispatch (see `ui.rs::should_continue`) simply starts the ladder over,
//! which is safe because it can only ever be *more* lenient, never less.

use std::collections::{HashMap, HashSet};

use forge_domain::{ToolCallArguments, ToolName};

/// Environment variable that enables the escalation ladder when set to `1`.
pub const ENV_VAR: &str = "FORGE_HARNESS_DOOM_LOOP_ESCALATION";

/// Whether the escalation ladder is enabled for this process.
pub fn enabled() -> bool {
    std::env::var(ENV_VAR).is_ok_and(|value| value == "1")
}

/// Occurrence count at which each response kicks in. Counts include the
/// call about to run, so `2` is the first repeat.
const SKIP_AT: usize = 3;
const PAUSE_AT: usize = 4;

/// Identifies one exact tool call: its name and canonicalized arguments.
/// `ToolCallArguments::parse()` collapses `Unparsed`/`Parsed` into the same
/// `serde_json::Value`, so formatting differences in the raw string don't
/// break the match; falls back to the raw string if parsing fails.
type Fingerprint = (ToolName, String);

fn fingerprint(name: &ToolName, arguments: &ToolCallArguments) -> Fingerprint {
    let canonical = arguments
        .clone()
        .parse()
        .map(|value| value.to_string())
        .unwrap_or_else(|_| arguments.clone().into_string());
    (name.clone(), canonical)
}

/// What to do with a tool call at its current repeat count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Escalation {
    /// First occurrence, or the flag is off: run normally, no warning.
    Allow,
    /// 1st repeat: run the tool, but the result carries a loud warning too.
    WarnAndRun { occurrences: usize },
    /// 2nd repeat: do not run the tool; return the warning as the result.
    WarnAndSkip { occurrences: usize },
    /// 3rd+ repeat: do not run the tool; pause the run for approval.
    PauseForApproval { occurrences: usize },
}

impl Escalation {
    /// Whether the tool should actually execute for this call.
    pub fn executes(&self) -> bool {
        matches!(self, Escalation::Allow | Escalation::WarnAndRun { .. })
    }
}

/// A tool call the run paused on, carried back to `Orchestrator::run` so it
/// can fire the `Interrupt` and stop the loop.
#[derive(Debug, Clone)]
pub struct PendingPause {
    pub tool_name: ToolName,
    pub occurrences: usize,
}

/// Per-run counters, keyed by fingerprint.
#[derive(Debug, Default, Clone)]
pub struct EscalationGuard {
    counts: HashMap<Fingerprint, usize>,
    /// Fingerprints that just paused the run. The *next* time this exact
    /// fingerprint is classified — whether that's a genuinely resumed run,
    /// or the same run continuing because an `on_end` hook (e.g.
    /// `PendingTodosHandler`) added messages and flipped `should_yield`
    /// back to `false` in the same `Orchestrator::run` call — it is let
    /// through once and the window restarts, instead of immediately
    /// re-triggering `PauseForApproval` on a count that never resets.
    armed: HashSet<Fingerprint>,
}

impl EscalationGuard {
    pub fn new() -> Self {
        Self::default()
    }

    /// Classifies the next call with this fingerprint and records it as
    /// having happened. Call once per tool call, immediately before
    /// deciding whether to execute it.
    pub fn classify(&mut self, name: &ToolName, arguments: &ToolCallArguments) -> Escalation {
        let key = fingerprint(name, arguments);

        if self.armed.remove(&key) {
            self.counts.insert(key, 1);
            return Escalation::Allow;
        }

        let occurrences = {
            let count = self.counts.entry(key.clone()).or_insert(0);
            *count += 1;
            *count
        };

        match occurrences {
            0 | 1 => Escalation::Allow,
            n if n < SKIP_AT => Escalation::WarnAndRun { occurrences: n },
            n if n < PAUSE_AT => Escalation::WarnAndSkip { occurrences: n },
            n => {
                self.armed.insert(key);
                Escalation::PauseForApproval { occurrences: n }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use forge_domain::{ToolCallArguments, ToolName};
    use pretty_assertions::assert_eq;

    use super::*;

    fn args(json: &str) -> ToolCallArguments {
        ToolCallArguments::from_json(json)
    }

    #[test]
    fn test_first_repeat_still_runs_with_a_warning() {
        let mut guard = EscalationGuard::new();
        let name = ToolName::new("shell");
        let arguments = args(r#"{"command": "cargo test"}"#);

        let first = guard.classify(&name, &arguments);
        let second = guard.classify(&name, &arguments);

        assert_eq!(first, Escalation::Allow);
        assert_eq!(second, Escalation::WarnAndRun { occurrences: 2 });
    }

    #[test]
    fn test_second_repeat_skips_execution() {
        let mut guard = EscalationGuard::new();
        let name = ToolName::new("shell");
        let arguments = args(r#"{"command": "cargo test"}"#);

        guard.classify(&name, &arguments); // 1st: Allow
        guard.classify(&name, &arguments); // 2nd: WarnAndRun
        let third = guard.classify(&name, &arguments);

        assert_eq!(third, Escalation::WarnAndSkip { occurrences: 3 });
    }

    #[test]
    fn test_third_repeat_pauses_then_one_shot_guard_lets_the_next_call_through() {
        let mut guard = EscalationGuard::new();
        let name = ToolName::new("shell");
        let arguments = args(r#"{"command": "cargo test"}"#);

        guard.classify(&name, &arguments); // 1st: Allow
        guard.classify(&name, &arguments); // 2nd: WarnAndRun
        guard.classify(&name, &arguments); // 3rd: WarnAndSkip
        let fourth = guard.classify(&name, &arguments);
        // The same call comes right back around after the pause.
        let fifth = guard.classify(&name, &arguments);

        assert_eq!(fourth, Escalation::PauseForApproval { occurrences: 4 });
        assert_eq!(fifth, Escalation::Allow); // one-shot guard consumed
    }

    #[test]
    fn test_different_arguments_do_not_accumulate() {
        let mut guard = EscalationGuard::new();
        let name = ToolName::new("read");

        guard.classify(&name, &args(r#"{"path": "a.txt"}"#));
        guard.classify(&name, &args(r#"{"path": "a.txt"}"#));
        let third = guard.classify(&name, &args(r#"{"path": "b.txt"}"#));

        assert_eq!(third, Escalation::Allow);
    }
}
