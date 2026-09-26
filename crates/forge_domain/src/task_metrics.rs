use std::collections::BTreeMap;

use derive_setters::Setters;
use serde::{Deserialize, Serialize};

use crate::{TokenCount, ToolName, Usage};

/// Reads the count out of a `TokenCount` regardless of whether the provider
/// reported it exactly or the runtime estimated it.
fn token_value(count: TokenCount) -> u64 {
    match count {
        TokenCount::Actual(value) | TokenCount::Approx(value) => value as u64,
    }
}

/// Token and call counts accumulated while completing a single task.
///
/// `Metrics` tracks *what* a session did (files touched, todos); this tracks
/// what it *cost*. Populated by the orchestrator as the loop runs and persisted
/// alongside the session metrics, so a completed task can be priced without
/// replaying it. Required by `R-EVAL-2`; consumed by the A/B runner.
///
/// Counts are cumulative over the task.
///
/// **Subagent work is not included.** The `task` tool builds a fresh
/// `Conversation` per subagent (`forge_app::agent_executor`), so its requests
/// and tokens accumulate on that conversation's own metrics and never reach the
/// parent. Only the parent's `task` tool *call* is counted, not what it cost.
/// Any A/B that changes subagent behaviour will therefore under-report tokens
/// until subagent metrics are rolled up; see `RECON.md`.
#[derive(Debug, Clone, Default, PartialEq, Setters, Serialize, Deserialize)]
#[setters(into, strip_option)]
#[serde(default)]
pub struct TaskMetrics {
    /// Number of completed requests to the model provider. A request that
    /// succeeded only after retries counts once, because retries are
    /// transport-level failures rather than reasoning steps.
    pub llm_calls: u64,

    /// Prompt tokens billed across every request, including the cached portion.
    pub input_tokens: u64,

    /// Subset of `input_tokens` served from the provider's prompt cache. Read
    /// against `input_tokens` to judge cache effectiveness (`R-CTX-8`).
    pub cached_input_tokens: u64,

    /// Completion tokens produced across every response.
    pub output_tokens: u64,

    /// Reasoning tokens billed by models that charge for extended thinking.
    ///
    /// Only populated for providers whose response carries it; zero elsewhere,
    /// which is indistinguishable from a model that did no reasoning. Treat a
    /// zero as "unknown" unless the provider is known to report it.
    pub reasoning_tokens: u64,

    /// Provider-reported cost in USD, summed across responses. `None` when no
    /// response carried a cost, which is not the same as a cost of zero.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<f64>,

    /// Completed tool calls per tool name, successes and failures alike.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub tool_calls: BTreeMap<String, u64>,

    /// Failed tool calls per tool name. A key here is always also present in
    /// `tool_calls`, so per-tool error rate is `tool_errors / tool_calls`.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub tool_errors: BTreeMap<String, u64>,

    /// Wall-clock milliseconds spent on the task, including provider latency
    /// and tool execution.
    pub wall_ms: u64,

    /// What compaction did over the course of the task.
    pub compactions: CompactionMetrics,
}

/// Aggregate record of every compaction performed during a task.
#[derive(Debug, Clone, Default, PartialEq, Setters, Serialize, Deserialize)]
#[setters(into, strip_option)]
#[serde(default)]
pub struct CompactionMetrics {
    /// How many times compaction ran.
    pub count: u64,

    /// Total tokens in the compacted ranges before compaction, summed across
    /// runs.
    pub tokens_before: u64,

    /// Total tokens those ranges occupied afterwards, summed across runs.
    /// `tokens_before - tokens_after` is the tokens reclaimed.
    pub tokens_after: u64,
}

impl CompactionMetrics {
    /// Records one compaction run.
    pub fn record(&mut self, tokens_before: u64, tokens_after: u64) {
        self.count += 1;
        self.tokens_before += tokens_before;
        self.tokens_after += tokens_after;
    }

    /// Tokens reclaimed across every run, saturating at zero if a compaction
    /// grew the context.
    pub fn tokens_saved(&self) -> u64 {
        self.tokens_before.saturating_sub(self.tokens_after)
    }
}

impl TaskMetrics {
    /// Records a completed provider request and the usage it reported.
    ///
    /// Call once per request with that request's own usage, already merged
    /// across stream events. Passing a usage value that is cumulative across
    /// requests would double-count.
    pub fn record_llm_call(&mut self, usage: Option<&Usage>) {
        self.llm_calls += 1;
        if let Some(usage) = usage {
            self.input_tokens += token_value(usage.prompt_tokens);
            self.output_tokens += token_value(usage.completion_tokens);
            self.cached_input_tokens += token_value(usage.cached_tokens);
            if let Some(cost) = usage.cost {
                self.cost = Some(self.cost.unwrap_or(0.0) + cost);
            }
        }
    }

    /// Records one completed tool call. `is_error` counts it as a failure in
    /// addition to, not instead of, counting it as a call.
    pub fn record_tool_call(&mut self, name: &ToolName, is_error: bool) {
        *self.tool_calls.entry(name.to_string()).or_default() += 1;
        if is_error {
            *self.tool_errors.entry(name.to_string()).or_default() += 1;
        }
    }

    /// Total tool calls across every tool.
    pub fn total_tool_calls(&self) -> u64 {
        self.tool_calls.values().sum()
    }

    /// Total failed tool calls across every tool.
    pub fn total_tool_errors(&self) -> u64 {
        self.tool_errors.values().sum()
    }

    /// Prompt tokens that were not served from cache.
    pub fn uncached_input_tokens(&self) -> u64 {
        self.input_tokens.saturating_sub(self.cached_input_tokens)
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    fn usage_fixture(prompt: usize, completion: usize, cached: usize) -> Usage {
        Usage {
            prompt_tokens: TokenCount::Actual(prompt),
            completion_tokens: TokenCount::Actual(completion),
            total_tokens: TokenCount::Actual(prompt + completion),
            cached_tokens: TokenCount::Actual(cached),
            cost: None,
        }
    }

    #[test]
    fn test_record_llm_call_accumulates_tokens() {
        let mut actual = TaskMetrics::default();
        actual.record_llm_call(Some(&usage_fixture(100, 20, 40)));
        actual.record_llm_call(Some(&usage_fixture(300, 50, 250)));

        let expected = TaskMetrics::default()
            .llm_calls(2u64)
            .input_tokens(400u64)
            .output_tokens(70u64)
            .cached_input_tokens(290u64);

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_record_llm_call_without_usage_still_counts_the_call() {
        let mut actual = TaskMetrics::default();
        actual.record_llm_call(None);

        let expected = TaskMetrics::default().llm_calls(1u64);

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_cost_is_none_until_a_response_reports_one() {
        let mut fixture = TaskMetrics::default();
        fixture.record_llm_call(Some(&usage_fixture(10, 1, 0)));
        assert_eq!(fixture.cost, None);

        let mut priced = usage_fixture(10, 1, 0);
        priced.cost = Some(0.25);
        fixture.record_llm_call(Some(&priced));
        fixture.record_llm_call(Some(&priced));

        assert_eq!(fixture.cost, Some(0.5));
    }

    #[test]
    fn test_record_tool_call_counts_errors_as_calls_too() {
        let mut actual = TaskMetrics::default();
        actual.record_tool_call(&ToolName::new("shell"), false);
        actual.record_tool_call(&ToolName::new("shell"), true);
        actual.record_tool_call(&ToolName::new("read"), false);

        let expected = TaskMetrics::default()
            .tool_calls(BTreeMap::from([
                ("shell".to_string(), 2u64),
                ("read".to_string(), 1u64),
            ]))
            .tool_errors(BTreeMap::from([("shell".to_string(), 1u64)]));

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_tool_call_totals() {
        let mut fixture = TaskMetrics::default();
        fixture.record_tool_call(&ToolName::new("shell"), true);
        fixture.record_tool_call(&ToolName::new("read"), false);
        fixture.record_tool_call(&ToolName::new("read"), true);

        assert_eq!(fixture.total_tool_calls(), 3);
        assert_eq!(fixture.total_tool_errors(), 2);
    }

    #[test]
    fn test_uncached_input_tokens() {
        let mut fixture = TaskMetrics::default();
        fixture.record_llm_call(Some(&usage_fixture(1000, 10, 750)));

        assert_eq!(fixture.uncached_input_tokens(), 250);
    }

    #[test]
    fn test_compaction_metrics_record() {
        let mut actual = CompactionMetrics::default();
        actual.record(5000, 800);
        actual.record(4000, 600);

        let expected = CompactionMetrics::default()
            .count(2u64)
            .tokens_before(9000u64)
            .tokens_after(1400u64);

        assert_eq!(actual, expected);
        assert_eq!(actual.tokens_saved(), 7600);
    }

    #[test]
    fn test_tokens_saved_saturates_when_compaction_grew_the_context() {
        let fixture = CompactionMetrics::default()
            .count(1u64)
            .tokens_before(100u64)
            .tokens_after(140u64);

        assert_eq!(fixture.tokens_saved(), 0);
    }

    #[test]
    fn test_default_serializes_to_the_minimal_object() {
        let fixture = TaskMetrics::default();
        let actual = serde_json::to_string(&fixture).unwrap();
        let expected = r#"{"llm_calls":0,"input_tokens":0,"cached_input_tokens":0,"output_tokens":0,"reasoning_tokens":0,"wall_ms":0,"compactions":{"count":0,"tokens_before":0,"tokens_after":0}}"#;

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_deserializes_from_an_object_missing_every_field() {
        let actual: TaskMetrics = serde_json::from_str("{}").unwrap();
        let expected = TaskMetrics::default();

        assert_eq!(actual, expected);
    }
}
