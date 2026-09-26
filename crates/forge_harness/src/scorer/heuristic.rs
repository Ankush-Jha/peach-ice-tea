//! `HeuristicScorer`: a relevance scorer with no model dependency.
//!
//! `DECISIONS.md` D-027 makes this the fallback whenever a model-backed
//! scorer errors, times out, or is disabled outside the evaluation profile —
//! it is deterministic, pure, and needs nothing beyond the call summaries
//! already in hand.

use super::plan::{decide, ResultStatus, ScoredCall, ScorerConfig, ToolCallSummary};
use super::RelevanceScorer;

/// Result sizes at or above this many characters are treated as "large" for
/// the purpose of the size penalty below: cheap to reconstruct the gist of
/// by truncating, expensive to keep verbatim.
const LARGE_RESULT_CHARS: f32 = 4_000.0;

/// A scorer that needs no model at all.
///
/// Scores are built from four factors, each documented on the field of the
/// formula it affects below: recency, result status, result size, and
/// whether the tool's result is cheap to reconstruct by calling it again.
/// The scorer holds no state and every call to `score` is a pure function
/// of its arguments, so results are reproducible and safe to use as the
/// fail-open fallback for any other scorer.
#[derive(Debug, Default, Clone, Copy)]
pub struct HeuristicScorer;

impl HeuristicScorer {
    /// Creates a new heuristic scorer.
    pub fn new() -> Self {
        Self
    }
}

/// Recency, normalised to `0.0` (the oldest call among `calls`) through
/// `1.0` (the newest). Later calls are more likely to still be relevant to
/// what the agent is doing now than earlier ones, so recency pushes both
/// `keep_call` and `keep_result` up. Falls back to the midpoint when every
/// call shares the same message index, so a single-call batch isn't biased
/// either way.
fn recency(message_index: usize, min_index: usize, max_index: usize) -> f32 {
    if max_index <= min_index {
        return 0.5;
    }
    (message_index - min_index) as f32 / (max_index - min_index) as f32
}

/// Whether a tool's result can be cheaply reconstructed by calling it again.
///
/// Reads, searches, listings and fetches are idempotent lookups against
/// state that already exists — re-reading a file or re-running a search
/// against an unchanged workspace gets the same answer — so their results
/// are safe to score lower than a tool whose output may not be reproducible
/// on demand, such as a `shell` command with side effects or one-off output
/// that won't recur.
fn is_recoverable_by_rerun(tool_name: &str) -> bool {
    let name = tool_name.to_ascii_lowercase();
    name.contains("read") || name.contains("search") || name.contains("fetch") || name.contains("list")
}

impl RelevanceScorer for HeuristicScorer {
    fn name(&self) -> &'static str {
        "heuristic"
    }

    fn score(&self, calls: &[ToolCallSummary], _goal: &str, config: &ScorerConfig) -> anyhow::Result<Vec<ScoredCall>> {
        let min_index = calls.iter().map(|call| call.message_index).min().unwrap_or(0);
        let max_index = calls.iter().map(|call| call.message_index).max().unwrap_or(0);

        let scored = calls
            .iter()
            .map(|call| {
                let recency_score = recency(call.message_index, min_index, max_index);
                let is_error = call.result_status == ResultStatus::Error;
                let recoverable = is_recoverable_by_rerun(&call.tool_name);

                // `keep_call` — does knowing this call ran still matter?
                // Recency is the dominant factor: recent activity is more
                // likely to still be in play than something from long ago.
                // An error is worth remembering happened even if its exact
                // bytes get truncated later, so it adds a modest flat bonus.
                let keep_call = (0.3 + 0.4 * recency_score + if is_error { 0.2 } else { 0.0 }).clamp(0.0, 1.0);

                // `keep_result` — must the full result stay verbatim?
                // Errors explain what went wrong and are worth keeping in
                // full, so they get a large bonus here (bigger than the
                // `keep_call` bonus, since a routine success's result is
                // much more disposable than an error's). Large results are
                // cheap to truncate down to their gist, so size counts
                // against them. Results that are cheap to reconstruct by
                // re-running the call (reads, searches, listings, fetches)
                // score lower than results that may not be reproducible on
                // demand.
                let size_penalty = (call.result_chars as f32 / LARGE_RESULT_CHARS).min(1.0) * 0.4;
                let recoverable_penalty = if recoverable { 0.25 } else { 0.0 };
                let error_bonus = if is_error { 0.4 } else { 0.0 };
                let keep_result =
                    (0.35 + 0.2 * recency_score + error_bonus - size_penalty - recoverable_penalty).clamp(0.0, 1.0);

                let decision = decide(Some(keep_call), Some(keep_result), false, config);

                ScoredCall {
                    call_id: call.call_id.clone(),
                    decision,
                    keep_call: Some(keep_call),
                    keep_result: Some(keep_result),
                    pinned: false,
                }
            })
            .collect();

        Ok(scored)
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    fn fixture_call(call_id: &str, result_status: ResultStatus, message_index: usize) -> ToolCallSummary {
        ToolCallSummary::new(call_id, "shell", "echo hi", result_status, 100, message_index)
    }

    #[test]
    fn test_an_error_result_outranks_a_routine_success_at_the_same_recency() {
        let calls = vec![fixture_call("error", ResultStatus::Error, 3), fixture_call("ok", ResultStatus::Ok, 3)];
        let config = ScorerConfig::default();
        let scorer = HeuristicScorer::new();

        let actual = scorer.score(&calls, "goal", &config).unwrap();

        let error_score = actual.iter().find(|scored| scored.call_id == "error").unwrap().keep_result.unwrap();
        let ok_score = actual.iter().find(|scored| scored.call_id == "ok").unwrap().keep_result.unwrap();
        assert!(error_score > ok_score, "error {error_score} should outrank success {ok_score}");
    }

    #[test]
    fn test_a_recent_call_outranks_an_old_call_at_the_same_status() {
        let calls = vec![fixture_call("old", ResultStatus::Ok, 0), fixture_call("recent", ResultStatus::Ok, 10)];
        let config = ScorerConfig::default();
        let scorer = HeuristicScorer::new();

        let actual = scorer.score(&calls, "goal", &config).unwrap();

        let old_score = actual.iter().find(|scored| scored.call_id == "old").unwrap().keep_call.unwrap();
        let recent_score = actual.iter().find(|scored| scored.call_id == "recent").unwrap().keep_call.unwrap();
        assert!(recent_score > old_score, "recent {recent_score} should outrank old {old_score}");
    }

    #[test]
    fn test_a_large_recoverable_read_scores_lower_for_keep_result_than_a_small_shell_result() {
        let large_read = ToolCallSummary::new("read", "fs_read", "path=/tmp/big.log", ResultStatus::Ok, 20_000, 5);
        let small_shell = ToolCallSummary::new("shell", "shell", "echo hi", ResultStatus::Ok, 20, 5);
        let calls = vec![large_read, small_shell];
        let config = ScorerConfig::default();
        let scorer = HeuristicScorer::new();

        let actual = scorer.score(&calls, "goal", &config).unwrap();

        let read_score = actual.iter().find(|scored| scored.call_id == "read").unwrap().keep_result.unwrap();
        let shell_score = actual.iter().find(|scored| scored.call_id == "shell").unwrap().keep_result.unwrap();
        assert!(read_score < shell_score, "large recoverable read {read_score} should score below small shell result {shell_score}");
    }

    #[test]
    fn test_scoring_is_deterministic() {
        let calls = vec![fixture_call("a", ResultStatus::Ok, 2), fixture_call("b", ResultStatus::Error, 4)];
        let config = ScorerConfig::default();
        let scorer = HeuristicScorer::new();

        let first = scorer.score(&calls, "goal", &config).unwrap();
        let second = scorer.score(&calls, "goal", &config).unwrap();

        assert_eq!(first, second);
    }

    #[test]
    fn test_scores_stay_within_the_zero_to_one_range() {
        let calls = vec![
            fixture_call("a", ResultStatus::Error, 100),
            ToolCallSummary::new("b", "fs_read", "x", ResultStatus::Ok, 1_000_000, 0),
        ];
        let config = ScorerConfig::default();
        let scorer = HeuristicScorer::new();

        let actual = scorer.score(&calls, "goal", &config).unwrap();

        for scored in &actual {
            let keep_call = scored.keep_call.unwrap();
            let keep_result = scored.keep_result.unwrap();
            assert!((0.0..=1.0).contains(&keep_call), "keep_call {keep_call} out of range");
            assert!((0.0..=1.0).contains(&keep_result), "keep_result {keep_result} out of range");
        }
    }
}
