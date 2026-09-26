use serde::{Deserialize, Serialize};

use crate::TaskMetrics;

/// How a non-interactive task ended.
///
/// Interactive mode can ask the user whether to continue after a limit is hit;
/// `exec` cannot, so each way a task can stop becomes a distinct, machine-
/// readable outcome. The A/B runner relies on this to tell "the agent finished
/// and was wrong" apart from "the agent never finished", which are different
/// results that a bare pass/fail would conflate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskOutcome {
    /// The model stopped of its own accord with no pending tool calls.
    Completed,
    /// Stopped because too many tool calls failed in one turn.
    ToolFailureLimit,
    /// Stopped because the turn hit its request ceiling.
    RequestLimit,
    /// Stopped because of an error: a provider failure, a bad configuration, or
    /// an I/O problem.
    Error,
}

impl TaskOutcome {
    /// Process exit code for this outcome.
    ///
    /// Distinct codes per failure mode so CI and the A/B runner can branch
    /// without parsing output. `1` stays the generic error, matching the
    /// convention the binary already used.
    pub fn exit_code(&self) -> i32 {
        match self {
            TaskOutcome::Completed => 0,
            TaskOutcome::Error => 1,
            TaskOutcome::ToolFailureLimit => 2,
            TaskOutcome::RequestLimit => 3,
        }
    }

    /// Whether the task ran to completion.
    pub fn is_success(&self) -> bool {
        matches!(self, TaskOutcome::Completed)
    }
}

/// The final structured line emitted by `exec`.
///
/// Printed as a single line of JSON to stdout so a caller can read the last
/// line without parsing the transcript above it. Required by `R-PROTO-7`;
/// consumed by the A/B runner (`R-EVAL-1`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecReport {
    /// How the task ended.
    pub outcome: TaskOutcome,

    /// Exit code the process will use, duplicated here so a caller that
    /// captured only stdout can still see it.
    pub exit_code: i32,

    /// What the task cost.
    pub metrics: TaskMetrics,

    /// Conversation the task ran in, for resuming or dumping it afterwards.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<String>,

    /// Model the task ran against, as the provider names it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Human-readable failure description. Present whenever `outcome` is not
    /// `Completed`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ExecReport {
    /// Builds a report, deriving `exit_code` from the outcome so the two can
    /// never disagree.
    pub fn new(outcome: TaskOutcome, metrics: TaskMetrics) -> Self {
        Self {
            outcome,
            exit_code: outcome.exit_code(),
            metrics,
            conversation_id: None,
            model: None,
            error: None,
        }
    }

    /// Sets the conversation the task ran in.
    pub fn conversation_id(mut self, id: impl ToString) -> Self {
        self.conversation_id = Some(id.to_string());
        self
    }

    /// Sets the model the task ran against.
    pub fn model(mut self, model: impl ToString) -> Self {
        self.model = Some(model.to_string());
        self
    }

    /// Sets the failure description.
    pub fn error(mut self, error: impl ToString) -> Self {
        self.error = Some(error.to_string());
        self
    }

    /// Serializes to the single JSON line `exec` prints.
    pub fn to_json_line(&self) -> serde_json::Result<String> {
        serde_json::to_string(self)
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_exit_codes_are_distinct_per_outcome() {
        let actual = [
            TaskOutcome::Completed.exit_code(),
            TaskOutcome::Error.exit_code(),
            TaskOutcome::ToolFailureLimit.exit_code(),
            TaskOutcome::RequestLimit.exit_code(),
        ];
        let expected = [0, 1, 2, 3];

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_only_completed_counts_as_success() {
        assert!(TaskOutcome::Completed.is_success());
        assert!(!TaskOutcome::Error.is_success());
        assert!(!TaskOutcome::ToolFailureLimit.is_success());
        assert!(!TaskOutcome::RequestLimit.is_success());
    }

    #[test]
    fn test_exit_code_field_matches_the_outcome() {
        let fixture = ExecReport::new(TaskOutcome::RequestLimit, TaskMetrics::default());

        assert_eq!(fixture.exit_code, TaskOutcome::RequestLimit.exit_code());
    }

    #[test]
    fn test_report_is_a_single_line_of_json() {
        let metrics = TaskMetrics::default().llm_calls(3u64);
        let fixture = ExecReport::new(TaskOutcome::Completed, metrics)
            .model("anthropic/claude-sonnet-4.6")
            .conversation_id("abc");

        let actual = fixture.to_json_line().unwrap();

        assert!(!actual.contains('\n'));
        let parsed: ExecReport = serde_json::from_str(&actual).unwrap();
        assert_eq!(parsed, fixture);
    }

    #[test]
    fn test_outcome_serializes_as_snake_case() {
        let fixture = ExecReport::new(TaskOutcome::ToolFailureLimit, TaskMetrics::default());
        let actual = fixture.to_json_line().unwrap();

        assert!(actual.contains(r#""outcome":"tool_failure_limit""#));
        assert!(actual.contains(r#""exit_code":2"#));
    }
}
