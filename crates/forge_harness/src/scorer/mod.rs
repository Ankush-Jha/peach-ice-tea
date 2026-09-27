//! The `RelevanceScorer` trait and the safety rules every implementation is
//! built on (`R-CTX-4`, `DECISIONS.md` D-027).
//!
//! This module builds `CompactionPlan`s from a scorer's answers; it does not
//! implement the model-backed scorers themselves (`LlmScorer`, `JevScorer`
//! are a later piece — `HeuristicScorer` in `heuristic.rs` is the only
//! implementation here today). Two rules are enforced here rather than left
//! to each scorer to remember:
//! - the first message and anything within the retention window of the
//!   newest message are pinned and never sent to a scorer at all;
//! - every `input_preview` is redacted (R-SAFE-3) before a scorer sees it,
//!   and a scorer error or a missing answer falls back to `Decision::Keep`
//!   for the affected calls rather than losing data (R-CTX-5).

use std::collections::HashMap;

pub mod external;
pub mod heuristic;
pub mod plan;

use plan::{decide, CompactionPlan, Decision, PlanStats, ScoredCall, ScorerConfig, ToolCallSummary};

use crate::redact;

/// Scores tool calls for relevance so a compaction stage can decide which
/// ones to keep verbatim, truncate, or drop along with their results.
///
/// Implementations are never handed pinned calls — `build_plan` filters
/// those out before calling `score` — and never see an unredacted
/// `input_preview`, since `build_plan` redacts it first.
pub trait RelevanceScorer {
    /// A short, stable identifier for logs and stats. Not shown to a model.
    fn name(&self) -> &'static str;

    /// Scores every call in `calls` against `goal`.
    ///
    /// Implementations should return one `ScoredCall` per input call,
    /// matched by `call_id`; `build_plan` treats any call missing from the
    /// result as if the scorer had failed for that call specifically, and
    /// keeps it rather than dropping it.
    ///
    /// # Errors
    /// Returns an error if scoring cannot be completed at all (model
    /// failure, timeout, malformed answer). Callers must treat this as
    /// "keep everything scored", never "drop everything" (R-CTX-5).
    fn score(&self, calls: &[ToolCallSummary], goal: &str, config: &ScorerConfig) -> anyhow::Result<Vec<ScoredCall>>;
}

/// Whether a call is pinned: the first message of the conversation, or
/// within `preserve_recent_messages` of the newest message seen among the
/// calls being planned.
fn is_pinned(message_index: usize, max_index: usize, preserve_recent_messages: usize) -> bool {
    if message_index == 0 {
        return true;
    }
    if preserve_recent_messages == 0 {
        return false;
    }
    let cutoff = max_index.saturating_sub(preserve_recent_messages - 1);
    message_index >= cutoff
}

/// Builds a compaction plan for `calls` using `scorer`.
///
/// Redacts every `input_preview` (R-SAFE-3), pins the first message and the
/// newest `config.preserve_recent_messages` messages without scoring them,
/// and asks `scorer` about everything else. If the scorer errors, or leaves
/// out an answer for a call it was asked about, that call falls back to
/// `Decision::Keep` and the reason is logged — compaction must fail open,
/// never silently delete a tool result (R-CTX-5, `CLAUDE.md` principle 5).
pub fn build_plan(
    scorer: &dyn RelevanceScorer,
    calls: &[ToolCallSummary],
    goal: &str,
    config: &ScorerConfig,
) -> CompactionPlan {
    let redacted: Vec<ToolCallSummary> = calls
        .iter()
        .cloned()
        .map(|call| ToolCallSummary { input_preview: redact::redact(&call.input_preview).into_owned(), ..call })
        .collect();

    let max_index = redacted.iter().map(|call| call.message_index).max().unwrap_or(0);
    let pinned_predicate =
        |call: &ToolCallSummary| is_pinned(call.message_index, max_index, config.preserve_recent_messages);

    let unpinned: Vec<ToolCallSummary> = redacted.iter().filter(|call| !pinned_predicate(call)).cloned().collect();

    let mut answers: HashMap<String, ScoredCall> = HashMap::new();
    if !unpinned.is_empty() {
        match scorer.score(&unpinned, goal, config) {
            Ok(scored) => {
                for entry in scored {
                    answers.insert(entry.call_id.clone(), entry);
                }
            }
            Err(error) => {
                tracing::warn!(
                    scorer = scorer.name(),
                    %error,
                    "Relevance scorer failed; keeping every unpinned call (fail open, R-CTX-5)"
                );
            }
        }
    }

    let scored_calls: Vec<ScoredCall> = redacted
        .iter()
        .map(|call| {
            if pinned_predicate(call) {
                ScoredCall {
                    call_id: call.call_id.clone(),
                    decision: decide(None, None, true, config),
                    keep_call: None,
                    keep_result: None,
                    pinned: true,
                }
            } else {
                answers.remove(&call.call_id).unwrap_or_else(|| {
                    tracing::warn!(
                        scorer = scorer.name(),
                        call_id = %call.call_id,
                        "Relevance scorer returned no answer for this call; keeping it (fail open, R-CTX-5)"
                    );
                    ScoredCall {
                        call_id: call.call_id.clone(),
                        decision: decide(None, None, false, config),
                        keep_call: None,
                        keep_result: None,
                        pinned: false,
                    }
                })
            }
        })
        .collect();

    let stats = compute_stats(calls, &scored_calls);
    CompactionPlan { scored: scored_calls, stats }
}

fn compute_stats(calls: &[ToolCallSummary], scored: &[ScoredCall]) -> PlanStats {
    let chars_before: usize = calls.iter().map(|call| call.result_chars).sum();
    let result_chars_by_id: HashMap<&str, usize> =
        calls.iter().map(|call| (call.call_id.as_str(), call.result_chars)).collect();

    let mut stats = PlanStats { total_calls: calls.len(), chars_before, ..PlanStats::default() };
    for entry in scored {
        if entry.pinned {
            stats.pinned += 1;
        }
        let result_chars = result_chars_by_id.get(entry.call_id.as_str()).copied().unwrap_or(0);
        match entry.decision {
            Decision::Keep => {
                stats.kept += 1;
                stats.chars_after_estimate += result_chars;
            }
            Decision::Truncate { head_chars } => {
                stats.truncated += 1;
                stats.chars_after_estimate += result_chars.min(head_chars);
            }
            Decision::Drop => {
                stats.dropped += 1;
            }
        }
    }
    stats
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use anyhow::anyhow;
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::scorer::plan::ResultStatus;

    /// A scorer that always fails, to prove `build_plan` fails open.
    struct FailingScorer;

    impl RelevanceScorer for FailingScorer {
        fn name(&self) -> &'static str {
            "failing"
        }

        fn score(&self, _calls: &[ToolCallSummary], _goal: &str, _config: &ScorerConfig) -> anyhow::Result<Vec<ScoredCall>> {
            Err(anyhow!("simulated scorer failure"))
        }
    }

    /// A scorer that records exactly what it was given, and answers nothing
    /// (used to prove pinning and redaction independently of scoring math).
    struct RecordingScorer {
        received: RefCell<Vec<ToolCallSummary>>,
    }

    impl RecordingScorer {
        fn new() -> Self {
            Self { received: RefCell::new(Vec::new()) }
        }
    }

    impl RelevanceScorer for RecordingScorer {
        fn name(&self) -> &'static str {
            "recording"
        }

        fn score(&self, calls: &[ToolCallSummary], _goal: &str, _config: &ScorerConfig) -> anyhow::Result<Vec<ScoredCall>> {
            self.received.borrow_mut().extend_from_slice(calls);
            Ok(Vec::new())
        }
    }

    fn call(call_id: &str, message_index: usize) -> ToolCallSummary {
        ToolCallSummary::new(call_id, "read", "path=/tmp/x", ResultStatus::Ok, 10, message_index)
    }

    #[test]
    fn test_a_failing_scorer_keeps_every_unpinned_call() {
        let calls = vec![call("c0", 0), call("c1", 1), call("c2", 2)];
        let config = ScorerConfig::default().preserve_recent_messages(0);

        let actual = build_plan(&FailingScorer, &calls, "goal", &config);

        assert!(actual.scored.iter().all(|scored| scored.decision == Decision::Keep));
        assert_eq!(actual.stats.dropped, 0);
        assert_eq!(actual.stats.truncated, 0);
        assert_eq!(actual.stats.kept, 3);
    }

    #[test]
    fn test_a_scorer_omitting_an_answer_still_keeps_that_call() {
        let calls = vec![call("c0", 0), call("c1", 1)];
        let config = ScorerConfig::default().preserve_recent_messages(0);
        let scorer = RecordingScorer::new(); // answers nothing for anyone

        let actual = build_plan(&scorer, &calls, "goal", &config);

        let missing = actual.scored.iter().find(|scored| scored.call_id == "c1").unwrap();
        assert_eq!(missing.decision, Decision::Keep);
        assert_eq!(missing.keep_call, None);
        assert_eq!(missing.keep_result, None);
    }

    #[test]
    fn test_the_first_message_and_recent_messages_are_pinned_and_never_scored() {
        let calls = vec![call("c0", 0), call("c1", 1), call("c2", 5), call("c3", 6)];
        let config = ScorerConfig::default().preserve_recent_messages(2);
        let scorer = RecordingScorer::new();

        let actual = build_plan(&scorer, &calls, "goal", &config);

        // Pinned: c0 (first message) and c3 (within the last 2 of newest
        // index 6, i.e. indices 5 and 6) -> c2 is also pinned, c1 is not.
        let pinned_ids: Vec<&str> =
            actual.scored.iter().filter(|scored| scored.pinned).map(|scored| scored.call_id.as_str()).collect();
        assert_eq!(pinned_ids, vec!["c0", "c2", "c3"]);

        let received_ids: Vec<String> =
            scorer.received.borrow().iter().map(|received| received.call_id.clone()).collect();
        assert_eq!(received_ids, vec!["c1".to_string()]);
    }

    #[test]
    fn test_input_preview_is_redacted_before_the_scorer_sees_it() {
        let secret_call =
            ToolCallSummary::new("c0", "shell", "curl -H 'api_key=sk-abcdefghijklmnopqrstuvwxyz'", ResultStatus::Ok, 10, 1);
        let calls = vec![secret_call];
        let config = ScorerConfig::default().preserve_recent_messages(0);
        let scorer = RecordingScorer::new();

        build_plan(&scorer, &calls, "goal", &config);

        let received = scorer.received.borrow();
        assert_eq!(received.len(), 1);
        assert!(!received[0].input_preview.contains("sk-abcdefghijklmnopqrstuvwxyz"));
        assert!(received[0].input_preview.contains("[REDACTED]"));
    }

    #[test]
    fn test_stats_count_pinned_kept_truncated_and_dropped() {
        let calls = vec![call("c0", 0).result_chars(1000)];
        let config = ScorerConfig::default();

        let actual = build_plan(&FailingScorer, &calls, "goal", &config);

        assert_eq!(actual.stats.total_calls, 1);
        assert_eq!(actual.stats.pinned, 1);
        assert_eq!(actual.stats.kept, 1);
        assert_eq!(actual.stats.chars_before, 1000);
        assert_eq!(actual.stats.chars_after_estimate, 1000);
    }
}
