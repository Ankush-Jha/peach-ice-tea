//! Data types for relevance-scored compaction (`R-CTX-4`).
//!
//! Nothing here does any I/O or calls a model; this is the shape of a
//! compaction plan and the one rule (`decide`) that turns a pair of
//! probabilities into a decision. `DECISIONS.md` D-027 governs the wider
//! design: `HeuristicScorer` today, an `LlmScorer` and `JevScorer` are later
//! pieces built on this same trait.

use serde::{Deserialize, Serialize};

/// Outcome of one tool call, used by scorers to weigh whether a result is
/// worth keeping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResultStatus {
    /// The tool call completed without error.
    Ok,
    /// The tool call failed, or its result itself represents an error.
    Error,
}

/// A compact, scoreable view of one tool call and its result.
///
/// Deliberately doesn't carry the full result body — scorers reason about
/// size, status, tool identity and recency, not result content — and
/// `input_preview` is redacted (`redact::redact`) before a `ToolCallSummary`
/// is ever built for a scorer to see (R-SAFE-3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCallSummary {
    /// Identifier pairing this call with its result, stable across the
    /// compaction pipeline.
    pub call_id: String,
    /// Name of the tool that was called.
    pub tool_name: String,
    /// A bounded, redacted preview of the call's input.
    pub input_preview: String,
    /// Whether the call's result was an error.
    pub result_status: ResultStatus,
    /// Length of the full (unredacted, un-truncated) result, in characters.
    pub result_chars: usize,
    /// Position of this call's message in the conversation, used to
    /// determine pinning and recency.
    pub message_index: usize,
}

impl ToolCallSummary {
    /// Creates a new summary of one tool call and its result.
    pub fn new(
        call_id: impl Into<String>,
        tool_name: impl Into<String>,
        input_preview: impl Into<String>,
        result_status: ResultStatus,
        result_chars: usize,
        message_index: usize,
    ) -> Self {
        Self {
            call_id: call_id.into(),
            tool_name: tool_name.into(),
            input_preview: input_preview.into(),
            result_status,
            result_chars,
            message_index,
        }
    }

    /// Sets the call identifier.
    pub fn call_id(mut self, call_id: impl Into<String>) -> Self {
        self.call_id = call_id.into();
        self
    }

    /// Sets the tool name.
    pub fn tool_name(mut self, tool_name: impl Into<String>) -> Self {
        self.tool_name = tool_name.into();
        self
    }

    /// Sets the input preview.
    pub fn input_preview(mut self, input_preview: impl Into<String>) -> Self {
        self.input_preview = input_preview.into();
        self
    }

    /// Sets the result status.
    pub fn result_status(mut self, result_status: ResultStatus) -> Self {
        self.result_status = result_status;
        self
    }

    /// Sets the full result length, in characters.
    pub fn result_chars(mut self, result_chars: usize) -> Self {
        self.result_chars = result_chars;
        self
    }

    /// Sets the message index.
    pub fn message_index(mut self, message_index: usize) -> Self {
        self.message_index = message_index;
        self
    }
}

/// What to do with one tool call and its result during compaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    /// Keep the call and its result verbatim.
    Keep,
    /// Keep the call, but bound its result to a head of `head_chars`
    /// characters plus a note (the note itself is applied by the caller
    /// that renders this decision, not by this type).
    Truncate {
        /// Number of leading characters of the result to keep.
        head_chars: usize,
    },
    /// Drop the call and its result together. Never split: a result is
    /// never dropped while its call is kept, or vice versa.
    Drop,
}

/// One call's scoring outcome and the decision derived from it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScoredCall {
    /// Identifier of the scored call, matching `ToolCallSummary::call_id`.
    pub call_id: String,
    /// What to do with this call and its result.
    pub decision: Decision,
    /// Probability that knowing this call ran still matters, if a scorer
    /// produced one. `None` when the call was pinned or the scorer gave no
    /// answer.
    pub keep_call: Option<f32>,
    /// Probability that the full result must stay verbatim because
    /// re-running the call wouldn't recover it, if a scorer produced one.
    pub keep_result: Option<f32>,
    /// Whether this call was pinned (first message, or within the retention
    /// window of the newest) and therefore never sent to a scorer.
    pub pinned: bool,
}

impl ScoredCall {
    /// Creates a new scored call with no probabilities recorded yet.
    pub fn new(call_id: impl Into<String>, decision: Decision) -> Self {
        Self { call_id: call_id.into(), decision, keep_call: None, keep_result: None, pinned: false }
    }

    /// Sets the call identifier.
    pub fn call_id(mut self, call_id: impl Into<String>) -> Self {
        self.call_id = call_id.into();
        self
    }

    /// Sets the decision.
    pub fn decision(mut self, decision: Decision) -> Self {
        self.decision = decision;
        self
    }

    /// Sets the "does knowing this call ran still matter" probability.
    pub fn keep_call(mut self, keep_call: f32) -> Self {
        self.keep_call = Some(keep_call);
        self
    }

    /// Sets the "must the result stay verbatim" probability.
    pub fn keep_result(mut self, keep_result: f32) -> Self {
        self.keep_result = Some(keep_result);
        self
    }

    /// Sets whether this call was pinned.
    pub fn pinned(mut self, pinned: bool) -> Self {
        self.pinned = pinned;
        self
    }
}

/// Aggregate counts and character totals for one compaction plan.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanStats {
    /// Number of calls the plan covers.
    pub total_calls: usize,
    /// Number of calls pinned and never scored.
    pub pinned: usize,
    /// Number of calls decided `Keep` (including pinned ones).
    pub kept: usize,
    /// Number of calls decided `Truncate`.
    pub truncated: usize,
    /// Number of calls decided `Drop`.
    pub dropped: usize,
    /// Total result characters across every call before compaction.
    pub chars_before: usize,
    /// Estimated total result characters after applying every decision
    /// (kept results in full, truncated results to their head length,
    /// dropped results as zero).
    pub chars_after_estimate: usize,
}

/// A full compaction plan: one decision per call, plus its stats.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CompactionPlan {
    /// One entry per input call, in the same order they were given.
    pub scored: Vec<ScoredCall>,
    /// Aggregate stats for the plan.
    pub stats: PlanStats,
}

/// Configuration for relevance scoring and the decisions built on it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ScorerConfig {
    /// Minimum probability for a `keep_call`/`keep_result` answer to count
    /// as "yes".
    pub threshold: f32,
    /// Number of most recent messages (plus the first message) that are
    /// pinned and never sent to a scorer.
    pub preserve_recent_messages: usize,
    /// Number of leading characters kept when a call is truncated rather
    /// than dropped.
    pub truncate_head_chars: usize,
}

impl Default for ScorerConfig {
    fn default() -> Self {
        Self { threshold: 0.5, preserve_recent_messages: 6, truncate_head_chars: 300 }
    }
}

impl ScorerConfig {
    /// Sets the keep threshold.
    pub fn threshold(mut self, threshold: f32) -> Self {
        self.threshold = threshold;
        self
    }

    /// Sets the number of pinned recent messages.
    pub fn preserve_recent_messages(mut self, preserve_recent_messages: usize) -> Self {
        self.preserve_recent_messages = preserve_recent_messages;
        self
    }

    /// Sets the truncation head length.
    pub fn truncate_head_chars(mut self, truncate_head_chars: usize) -> Self {
        self.truncate_head_chars = truncate_head_chars;
        self
    }
}

/// Implements save-token-jev's decision step (S2, step 5) exactly.
///
/// A pinned call always wins and is kept, regardless of its scores.
/// Otherwise: a `keep_result` at or above `config.threshold` keeps the call
/// and its result verbatim; failing that, a `keep_call` at or above
/// `config.threshold` truncates the result to `config.truncate_head_chars`;
/// failing that, the call and its result are dropped together. A missing
/// score — `keep_result` or `keep_call` is `None` — always defaults to
/// `Keep`, never `Drop`: a scorer that errors, times out, or omits an
/// answer must never be the reason a tool result is silently deleted
/// (R-CTX-5, `CLAUDE.md` principle 5, fail open).
pub fn decide(keep_call: Option<f32>, keep_result: Option<f32>, pinned: bool, config: &ScorerConfig) -> Decision {
    if pinned {
        return Decision::Keep;
    }

    match keep_result {
        None => return Decision::Keep,
        Some(score) if score >= config.threshold => return Decision::Keep,
        Some(_) => {}
    }

    match keep_call {
        None => Decision::Keep,
        Some(score) if score >= config.threshold => Decision::Truncate { head_chars: config.truncate_head_chars },
        Some(_) => Decision::Drop,
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_pinned_always_keeps_regardless_of_scores() {
        let config = ScorerConfig::default();

        let actual = decide(Some(0.0), Some(0.0), true, &config);

        assert_eq!(actual, Decision::Keep);
    }

    #[test]
    fn test_keep_result_at_or_above_threshold_keeps() {
        let config = ScorerConfig::default();

        let actual = decide(Some(0.0), Some(0.5), false, &config);

        assert_eq!(actual, Decision::Keep);
    }

    #[test]
    fn test_keep_result_below_threshold_and_keep_call_at_or_above_threshold_truncates() {
        let config = ScorerConfig::default();

        let actual = decide(Some(0.5), Some(0.49), false, &config);

        assert_eq!(actual, Decision::Truncate { head_chars: 300 });
    }

    #[test]
    fn test_truncate_uses_the_configured_head_length() {
        let config = ScorerConfig::default().truncate_head_chars(50);

        let actual = decide(Some(0.9), Some(0.1), false, &config);

        assert_eq!(actual, Decision::Truncate { head_chars: 50 });
    }

    #[test]
    fn test_both_scores_below_threshold_drops() {
        let config = ScorerConfig::default();

        let actual = decide(Some(0.1), Some(0.2), false, &config);

        assert_eq!(actual, Decision::Drop);
    }

    #[test]
    fn test_missing_keep_result_defaults_to_keep_never_drop() {
        let config = ScorerConfig::default();

        let actual = decide(Some(0.0), None, false, &config);

        assert_eq!(actual, Decision::Keep);
    }

    #[test]
    fn test_missing_keep_call_after_low_keep_result_defaults_to_keep_never_drop() {
        let config = ScorerConfig::default();

        let actual = decide(None, Some(0.0), false, &config);

        assert_eq!(actual, Decision::Keep);
    }

    #[test]
    fn test_both_scores_missing_defaults_to_keep() {
        let config = ScorerConfig::default();

        let actual = decide(None, None, false, &config);

        assert_eq!(actual, Decision::Keep);
    }

    #[test]
    fn test_default_config_matches_the_ported_algorithm() {
        let actual = ScorerConfig::default();

        let expected = ScorerConfig { threshold: 0.5, preserve_recent_messages: 6, truncate_head_chars: 300 };
        assert_eq!(actual, expected);
    }
}
