//! harness: R-LOOP-3 (D-097) — reasoning spent in proportion to how hard the
//! task turns out to be, not how hard it might be.
//!
//! Every run starts cheap: `low` effort, and optionally a cheaper model. Most
//! hackathon-sized fixes (read, patch, run the tests) never need more. The
//! run escalates once, and for good, when the task itself shows it is hard:
//! the agent's tests fail on the code, the runtime gate had to send a stop
//! back, tools keep failing, or the task runs long without a green run. It
//! then uses `high` effort, and the stronger model if one is named. The
//! evidence comes from what the harness already observes, never from asking
//! the model to rate its own task.
//!
//! This departs from the spec's fixed schedule (high for the first ten
//! messages, then low): on these tasks the early calls are the cheap ones
//! (reading two files), and the later ones, after a failure, need thought.
//!
//! Off unless [`ENV_VAR`] is `1` (principle 6). A model without a reasoning
//! config is left alone (fail open, principle 5).

use peach_domain::{Context, Effort, ModelId, TaskMetrics};

/// Enables the schedule when `1`.
pub const ENV_VAR: &str = "PEACH_HARNESS_REASONING_SCHEDULE";

/// A stronger model to switch to on escalation (same provider), e.g. start on
/// a fast model via `MODEL=` and escalate to a thinking one.
pub const ESCALATE_MODEL_VAR: &str = "PEACH_HARNESS_ESCALATE_MODEL";

/// Model calls without a passing test run after which a task counts as hard.
const CALLS_BEFORE_ESCALATION: u64 = 12;

/// Tool errors after which a task counts as hard.
const TOOL_ERRORS_BEFORE_ESCALATION: u64 = 2;

/// Whether the schedule is on for this process.
pub fn enabled() -> bool {
    std::env::var(ENV_VAR).is_ok_and(|value| value == "1")
}

/// The model to escalate to, when one is configured.
pub fn escalation_model() -> Option<ModelId> {
    std::env::var(ESCALATE_MODEL_VAR).ok().filter(|v| !v.trim().is_empty()).map(|v| ModelId::new(v.trim()))
}

/// What the harness has observed about the task's difficulty so far.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Difficulty {
    /// Agent test runs that failed on the code (not the environment).
    pub failed_test_runs: u32,
    /// Times the runtime gate had to check a stop.
    pub gate_runs: u32,
    /// Tool calls that failed.
    pub tool_errors: u64,
    /// Model calls so far.
    pub llm_calls: u64,
}

impl Difficulty {
    /// Reads the signals from the task's metrics and the verification state.
    ///
    /// # Arguments
    /// * `task` - The task's metrics so far.
    pub fn observe(task: &TaskMetrics) -> Self {
        let (failed_test_runs, gate_runs) = peach_harness::verify::difficulty_signals();
        Self { failed_test_runs, gate_runs, tool_errors: task.tool_errors.values().sum(), llm_calls: task.llm_calls }
    }

    /// Why the task now counts as hard, or `None` while it still looks simple.
    pub fn escalation_reason(&self) -> Option<String> {
        if self.failed_test_runs > 0 {
            Some(format!("{} failing test run(s)", self.failed_test_runs))
        } else if self.gate_runs > 0 {
            Some("the runtime gate had to check a stop".to_string())
        } else if self.tool_errors >= TOOL_ERRORS_BEFORE_ESCALATION {
            Some(format!("{} tool errors", self.tool_errors))
        } else if self.llm_calls >= CALLS_BEFORE_ESCALATION {
            Some(format!("{} model calls without finishing", self.llm_calls))
        } else {
            None
        }
    }
}

/// The effort for the next request.
///
/// # Arguments
/// * `escalated` - Whether the task has proven hard.
pub fn effort(escalated: bool) -> Effort {
    if escalated { Effort::High } else { Effort::Low }
}

/// `context` with its reasoning effort set to `effort`. Unchanged when the
/// model has no reasoning config or reasoning is switched off.
///
/// # Arguments
/// * `context` - The request context.
/// * `effort` - The effort to request.
pub fn with_effort(mut context: Context, effort: Effort) -> Context {
    if let Some(reasoning) = context.reasoning.as_mut()
        && reasoning.enabled != Some(false)
    {
        reasoning.effort = Some(effort);
    }
    context
}

#[cfg(test)]
mod tests {
    use peach_domain::ReasoningConfig;
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_a_simple_task_never_escalates_and_evidence_of_difficulty_does() {
        let simple = Difficulty { llm_calls: 5, ..Default::default() };
        let cases = [
            Difficulty { failed_test_runs: 1, ..simple },
            Difficulty { gate_runs: 1, ..simple },
            Difficulty { tool_errors: 2, ..simple },
            Difficulty { llm_calls: 12, ..simple },
        ];

        let actual: Vec<bool> = cases.iter().map(|d| d.escalation_reason().is_some()).collect();

        assert_eq!(simple.escalation_reason(), None);
        assert_eq!(actual, vec![true, true, true, true]);
    }

    #[test]
    fn test_effort_is_set_only_where_reasoning_is_on() {
        let on = Context::default().reasoning(ReasoningConfig::default().enabled(true));
        let off = Context::default().reasoning(ReasoningConfig::default().enabled(false));

        let actual = (
            with_effort(on, Effort::Low).reasoning.and_then(|r| r.effort),
            with_effort(off, Effort::Low).reasoning.and_then(|r| r.effort),
            with_effort(Context::default(), Effort::Low).reasoning,
        );

        assert_eq!(actual, (Some(Effort::Low), None, None));
    }
}
