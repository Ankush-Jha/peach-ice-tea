use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

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
/// **Subagent work is included.** The `task` tool builds a fresh `Conversation`
/// per subagent, so its costs land on that conversation's own metrics; the
/// agent executor folds them back into the parent through `absorb_subagent`
/// once the subagent finishes. Without that rollup an A/B on subagent
/// behaviour — which is exactly what `R-LOOP-1` changes — would under-report
/// tokens. `wall_ms` is the one field that does not roll up; see
/// `absorb_subagent`.
#[derive(Debug, Clone, Default, PartialEq, Setters, Serialize, Deserialize)]
#[setters(into, strip_option)]
#[serde(default)]
pub struct TaskMetrics {
    /// Number of completed requests to the model provider. A request that
    /// succeeded only after retries counts once, because retries are
    /// transport-level failures rather than reasoning steps.
    pub llm_calls: u64,

    /// Requests that never produced a response: a non-retryable provider error,
    /// or retries exhausted. Counted separately from `llm_calls` because no
    /// usage is reported for them, so they cannot contribute tokens — but they
    /// cost wall time and may still have been billed provider-side.
    pub failed_llm_calls: u64,

    /// harness: R-HACK-3 — provider attempts that failed and were retried
    /// (rate limits, 5xx, empty completions). Not in `llm_calls` or
    /// `failed_llm_calls`, since the request eventually succeeded or failed
    /// once as a whole. Without this, a run that spent much of its wall time
    /// in retry backoff reported none of it (D-032). Some of these, such as
    /// empty completions, may still have been billed.
    #[serde(default)]
    pub retried_llm_calls: u64,

    /// Prompt tokens billed across every request, including the cached portion.
    pub input_tokens: u64,

    /// Subset of `input_tokens` served from the provider's prompt cache. Read
    /// against `input_tokens` to judge cache effectiveness (`R-CTX-8`).
    pub cached_input_tokens: u64,

    /// Completion tokens produced across every response.
    pub output_tokens: u64,

    /// Reasoning tokens billed by models that charge for extended thinking.
    ///
    /// Populated for OpenAI-shaped providers, which report it under
    /// `completion_tokens_details` (Chat Completions) or
    /// `output_tokens_details` (Responses). Anthropic's native API folds
    /// thinking into its output tokens and reports no separate figure, so this
    /// reads zero there — indistinguishable from a model that did no
    /// reasoning. Treat zero as "unknown" unless the provider is known to
    /// report it.
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

    /// Signals that the agent needed a turn to recover from something the
    /// harness withheld or repeated (R-EVAL-2, R-OUT-3).
    #[serde(default, skip_serializing_if = "RecoveryEvents::is_default")]
    pub recovery: RecoveryEvents,

    /// Truncation dump-file paths (`tool_executor.rs::dump_operation`) that
    /// have already been credited to `recovery.offload_read`. Re-reading the
    /// same dump file again is not a fresh recovery, so it is not counted
    /// twice. Process-local bookkeeping only; not persisted, since a resumed
    /// conversation degrading to under-counting is safer than the complexity
    /// of restoring it (CLAUDE.md principle 5, fail open).
    #[setters(skip)]
    #[serde(skip)]
    dump_files_read: HashSet<String>,

    /// Every dump-file path created for this task so far, so a later read of
    /// one of them can be recognised as `offload_read`. Process-local only,
    /// same reasoning as `dump_files_read`.
    #[setters(skip)]
    #[serde(skip)]
    dump_files: HashSet<String>,

    /// The most recent line range read for each file path, so a second read
    /// of the same file and range (with no write in between) can be
    /// recognised as `reread_same_range`. A write to a path clears its entry.
    /// Process-local only, same reasoning as `dump_files_read`.
    #[setters(skip)]
    #[serde(skip)]
    recent_reads: HashMap<String, (u64, u64)>,

    /// Shell commands run within the last `RERUN_WINDOW_CALLS` LLM calls,
    /// paired with the `llm_calls` count at the time they ran. Pruned on
    /// every call so it never grows past the window, regardless of task
    /// length. Process-local only, same reasoning as `dump_files_read`.
    #[setters(skip)]
    #[serde(skip)]
    recent_shell_commands: VecDeque<(u64, String)>,
}

/// Recovery signals counted while completing a task (R-EVAL-2).
///
/// Each field is a distinct way the agent spent a turn recovering from
/// something the harness withheld, repeated, or got wrong the first time.
/// `first_error_recovered` is derived when the exec report is built
/// (`ExecReport::new`: task completed with `tool_errors > 0`), not counted here.
#[derive(Debug, Clone, Default, PartialEq, Setters, Serialize, Deserialize)]
#[setters(into, strip_option)]
#[serde(default)]
pub struct RecoveryEvents {
    /// The agent read a path that was one of the harness's own truncation
    /// dump files (`tool_executor.rs::dump_operation`) — a turn spent
    /// recovering content the harness withheld (R-OUT-3).
    pub offload_read: u64,

    /// An identical shell command ran again within the last five LLM calls.
    pub rerun_same_command: u64,

    /// The same file and line range was read twice with no write to that
    /// file in between.
    pub reread_same_range: u64,

    /// The task completed after its first tool error. Set by
    /// `ExecReport::new`, which has the outcome and `tool_errors` together.
    pub first_error_recovered: u64,
}

impl RecoveryEvents {
    /// Whether every counter is at its default (zero), used to keep a task
    /// with no recovery events out of the serialized JSON.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// Adds a subagent's recovery counts into this task's totals, matching
    /// how `TaskMetrics::absorb_subagent` folds in every other counter.
    fn absorb(&mut self, child: &RecoveryEvents) {
        self.offload_read += child.offload_read;
        self.rerun_same_command += child.rerun_same_command;
        self.reread_same_range += child.reread_same_range;
        self.first_error_recovered += child.first_error_recovered;
    }

    /// What was recorded since `baseline`, field by field, saturating at
    /// zero like the rest of `TaskMetrics::since`.
    fn since(&self, baseline: &RecoveryEvents) -> RecoveryEvents {
        RecoveryEvents {
            offload_read: self.offload_read.saturating_sub(baseline.offload_read),
            rerun_same_command: self
                .rerun_same_command
                .saturating_sub(baseline.rerun_same_command),
            reread_same_range: self
                .reread_same_range
                .saturating_sub(baseline.reread_same_range),
            first_error_recovered: self
                .first_error_recovered
                .saturating_sub(baseline.first_error_recovered),
        }
    }
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
            self.reasoning_tokens += token_value(usage.reasoning_tokens);
            if let Some(cost) = usage.cost {
                self.cost = Some(self.cost.unwrap_or(0.0) + cost);
            }
        }
    }

    /// Records a request that failed without producing a response.
    pub fn record_failed_llm_call(&mut self) {
        self.failed_llm_calls += 1;
    }

    /// Adds the tokens of a failed attempt the provider still reported usage
    /// for (an empty completion) to the task's token totals. Not a call:
    /// `llm_calls` stays one per request, however many attempts it took.
    pub fn record_retried_usage(&mut self, usage: &Usage) {
        self.input_tokens += token_value(usage.prompt_tokens);
        self.output_tokens += token_value(usage.completion_tokens);
        self.cached_input_tokens += token_value(usage.cached_tokens);
        self.reasoning_tokens += token_value(usage.reasoning_tokens);
        if let Some(cost) = usage.cost {
            self.cost = Some(self.cost.unwrap_or(0.0) + cost);
        }
    }

    /// Records provider attempts that failed and were retried.
    pub fn record_retried_llm_calls(&mut self, count: u64) {
        self.retried_llm_calls += count;
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

    /// How many LLM calls back an identical shell command still counts as a
    /// rerun (R-EVAL-2).
    const RERUN_WINDOW_CALLS: u64 = 5;

    /// Records a temp-file path created to hold output a truncation withheld
    /// (`tool_executor.rs::dump_operation`), so a later read of that same
    /// path is recognised by `record_read` as `recovery.offload_read`
    /// (R-OUT-3).
    pub fn record_dump_file(&mut self, path: impl Into<String>) {
        self.dump_files.insert(path.into());
    }

    /// Adds `other`'s dump files to these. Hooks register handles (recall
    /// handles, D-066) on the conversation's copy of the metrics, while the
    /// tool executor counts reads against the tool context's copy.
    pub fn absorb_dump_files(&mut self, other: &TaskMetrics) {
        self.dump_files.extend(other.dump_files.iter().cloned());
    }

    /// Records a file read and updates the recovery counters it triggers.
    ///
    /// `recovery.offload_read` fires the first time a path known to be one
    /// of this task's own truncation dump files is read; reading the same
    /// dump file again does not fire it a second time, since the recovery
    /// already happened on the first read.
    ///
    /// `recovery.reread_same_range` fires when `path` was already the most
    /// recently read range for this exact `(start_line, end_line)`, with no
    /// `record_write` for `path` in between.
    pub fn record_read(&mut self, path: &str, start_line: u64, end_line: u64) {
        if self.dump_files.contains(path) && self.dump_files_read.insert(path.to_string()) {
            self.recovery.offload_read += 1;
        }

        let range = (start_line, end_line);
        if self.recent_reads.get(path) == Some(&range) {
            self.recovery.reread_same_range += 1;
        }
        self.recent_reads.insert(path.to_string(), range);
    }

    /// Clears the recorded read range for `path`, so a write breaks the
    /// `reread_same_range` chain for that file. Call for every write, patch,
    /// undo or remove.
    pub fn record_write(&mut self, path: &str) {
        self.recent_reads.remove(path);
    }

    /// Records a shell command and updates `recovery.rerun_same_command` if
    /// it repeats one already run within the last `RERUN_WINDOW_CALLS` LLM
    /// calls. Uses `llm_calls` as the turn index, per R-EVAL-2, and prunes
    /// the history to that window on every call so it stays bounded for the
    /// whole task regardless of how many shell calls it makes.
    pub fn record_shell_command(&mut self, command: &str) {
        let now = self.llm_calls;
        self.recent_shell_commands
            .retain(|(call, _)| now.saturating_sub(*call) < Self::RERUN_WINDOW_CALLS);

        if self
            .recent_shell_commands
            .iter()
            .any(|(_, cmd)| cmd == command)
        {
            self.recovery.rerun_same_command += 1;
        }
        self.recent_shell_commands.push_back((now, command.to_string()));
    }

    /// Folds a subagent's costs into this task's totals.
    ///
    /// Counts, tokens, cost and compactions add. `wall_ms` deliberately does
    /// not: subagents run concurrently with each other and with the parent, so
    /// summing their elapsed time would exceed the wall time actually spent.
    /// The parent's own clock already covers the period they ran in.
    pub fn absorb_subagent(&mut self, child: &TaskMetrics) {
        self.llm_calls += child.llm_calls;
        self.failed_llm_calls += child.failed_llm_calls;
        self.retried_llm_calls += child.retried_llm_calls;
        self.input_tokens += child.input_tokens;
        self.cached_input_tokens += child.cached_input_tokens;
        self.output_tokens += child.output_tokens;
        self.reasoning_tokens += child.reasoning_tokens;
        if let Some(child_cost) = child.cost {
            self.cost = Some(self.cost.unwrap_or(0.0) + child_cost);
        }
        for (tool, count) in child.tool_calls.iter() {
            *self.tool_calls.entry(tool.clone()).or_default() += count;
        }
        for (tool, count) in child.tool_errors.iter() {
            *self.tool_errors.entry(tool.clone()).or_default() += count;
        }
        self.compactions.count += child.compactions.count;
        self.compactions.tokens_before += child.compactions.tokens_before;
        self.compactions.tokens_after += child.compactions.tokens_after;
        self.recovery.absorb(&child.recovery);
    }

    /// Returns what was spent since `baseline`, field by field.
    ///
    /// A subagent's conversation metrics are cumulative across every run of
    /// that conversation. When a conversation is resumed, absorbing the whole
    /// total would count its earlier runs again, so the caller snapshots the
    /// metrics before the run and absorbs only this difference.
    ///
    /// Subtraction saturates at zero: metrics only ever grow, so a negative
    /// difference means the baseline did not belong to this conversation, and
    /// counting zero is safer than wrapping.
    pub fn since(&self, baseline: &TaskMetrics) -> TaskMetrics {
        let map_delta = |after: &BTreeMap<String, u64>, before: &BTreeMap<String, u64>| {
            after
                .iter()
                .filter_map(|(tool, count)| {
                    let delta = count.saturating_sub(before.get(tool).copied().unwrap_or(0));
                    (delta > 0).then(|| (tool.clone(), delta))
                })
                .collect()
        };

        TaskMetrics {
            llm_calls: self.llm_calls.saturating_sub(baseline.llm_calls),
            failed_llm_calls: self.failed_llm_calls.saturating_sub(baseline.failed_llm_calls),
            retried_llm_calls: self.retried_llm_calls.saturating_sub(baseline.retried_llm_calls),
            input_tokens: self.input_tokens.saturating_sub(baseline.input_tokens),
            cached_input_tokens: self
                .cached_input_tokens
                .saturating_sub(baseline.cached_input_tokens),
            output_tokens: self.output_tokens.saturating_sub(baseline.output_tokens),
            reasoning_tokens: self.reasoning_tokens.saturating_sub(baseline.reasoning_tokens),
            cost: match (self.cost, baseline.cost) {
                (Some(after), Some(before)) => Some((after - before).max(0.0)),
                (after, _) => after,
            },
            tool_calls: map_delta(&self.tool_calls, &baseline.tool_calls),
            tool_errors: map_delta(&self.tool_errors, &baseline.tool_errors),
            // Wall time is not absorbed from subagents, so a delta is meaningless.
            wall_ms: 0,
            compactions: CompactionMetrics {
                count: self.compactions.count.saturating_sub(baseline.compactions.count),
                tokens_before: self
                    .compactions
                    .tokens_before
                    .saturating_sub(baseline.compactions.tokens_before),
                tokens_after: self
                    .compactions
                    .tokens_after
                    .saturating_sub(baseline.compactions.tokens_after),
            },
            recovery: self.recovery.since(&baseline.recovery),
            // Process-local bookkeeping, not a counter: a delta has nothing
            // meaningful to subtract, so it starts fresh like `wall_ms`.
            dump_files_read: HashSet::new(),
            dump_files: HashSet::new(),
            recent_reads: HashMap::new(),
            recent_shell_commands: VecDeque::new(),
        }
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
            reasoning_tokens: TokenCount::Actual(0),
            cost: None,
        }
    }

    #[test]
    fn test_retries_add_billed_tokens_but_not_calls() {
        let mut fixture = TaskMetrics::default();
        fixture.record_llm_call(Some(&Usage {
            prompt_tokens: TokenCount::Actual(100),
            completion_tokens: TokenCount::Actual(10),
            ..Usage::default()
        }));

        fixture.record_retried_llm_calls(3);
        fixture.record_retried_usage(&Usage {
            prompt_tokens: TokenCount::Actual(100),
            completion_tokens: TokenCount::Actual(7),
            ..Usage::default()
        });

        let actual = (
            fixture.llm_calls,
            fixture.retried_llm_calls,
            fixture.input_tokens,
            fixture.output_tokens,
        );
        assert_eq!(actual, (1, 3, 200, 17));
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
    fn test_reasoning_tokens_accumulate() {
        let mut fixture = usage_fixture(10, 200, 0);
        fixture.reasoning_tokens = TokenCount::Actual(150);

        let mut actual = TaskMetrics::default();
        actual.record_llm_call(Some(&fixture));
        actual.record_llm_call(Some(&fixture));

        assert_eq!(actual.reasoning_tokens, 300);
    }

    #[test]
    fn test_absorb_subagent_adds_costs_but_not_wall_time() {
        let mut child = TaskMetrics::default();
        child.record_llm_call(Some(&usage_fixture(500, 120, 100)));
        child.record_tool_call(&ToolName::new("shell"), true);
        child.wall_ms = 9_000;
        child.compactions.record(2000, 300);

        let mut actual = TaskMetrics::default();
        actual.record_llm_call(Some(&usage_fixture(100, 10, 0)));
        actual.record_tool_call(&ToolName::new("shell"), false);
        actual.wall_ms = 4_000;
        actual.absorb_subagent(&child);

        let mut expected = TaskMetrics::default()
            .llm_calls(2u64)
            .input_tokens(600u64)
            .output_tokens(130u64)
            .cached_input_tokens(100u64)
            .wall_ms(4_000u64)
            .tool_calls(BTreeMap::from([("shell".to_string(), 2u64)]))
            .tool_errors(BTreeMap::from([("shell".to_string(), 1u64)]));
        expected.compactions.record(2000, 300);

        assert_eq!(actual, expected);
    }

    /// A resumed subagent conversation carries its earlier runs' totals.
    /// Absorbing the whole total counts them again; absorbing the delta does
    /// not. Reproduces the case an adversarial review caught.
    #[test]
    fn test_absorbing_a_resumed_subagent_counts_only_the_new_run() {
        // First run of the subagent: 1 call, 500 input tokens.
        let mut child_after_run_one = TaskMetrics::default();
        child_after_run_one.record_llm_call(Some(&usage_fixture(500, 0, 0)));

        let mut parent = TaskMetrics::default();
        parent.absorb_subagent(&child_after_run_one.since(&TaskMetrics::default()));

        // The conversation is resumed; its metrics are now cumulative.
        let mut child_after_run_two = child_after_run_one.clone();
        child_after_run_two.record_llm_call(Some(&usage_fixture(300, 0, 0)));

        parent.absorb_subagent(&child_after_run_two.since(&child_after_run_one));

        let expected = TaskMetrics::default()
            .llm_calls(2u64)
            .input_tokens(800u64);

        assert_eq!(parent, expected);
    }

    #[test]
    fn test_since_saturates_and_drops_unchanged_tools() {
        let mut baseline = TaskMetrics::default();
        baseline.record_llm_call(Some(&usage_fixture(100, 10, 0)));
        baseline.record_tool_call(&ToolName::new("shell"), false);

        let mut after = baseline.clone();
        after.record_tool_call(&ToolName::new("read"), true);

        let actual = after.since(&baseline);

        let expected = TaskMetrics::default()
            .tool_calls(BTreeMap::from([("read".to_string(), 1u64)]))
            .tool_errors(BTreeMap::from([("read".to_string(), 1u64)]));

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_since_a_foreign_baseline_saturates_to_zero() {
        let fixture = TaskMetrics::default().llm_calls(1u64).input_tokens(10u64);
        let larger = TaskMetrics::default().llm_calls(9u64).input_tokens(900u64);

        let actual = fixture.since(&larger);

        assert_eq!(actual, TaskMetrics::default());
    }

    #[test]
    fn test_failed_calls_are_counted_separately_from_completed_ones() {
        let mut actual = TaskMetrics::default();
        actual.record_llm_call(Some(&usage_fixture(10, 5, 0)));
        actual.record_failed_llm_call();
        actual.record_failed_llm_call();

        let expected = TaskMetrics::default()
            .llm_calls(1u64)
            .failed_llm_calls(2u64)
            .input_tokens(10u64)
            .output_tokens(5u64);

        assert_eq!(actual, expected);
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
        let expected = r#"{"llm_calls":0,"failed_llm_calls":0,"retried_llm_calls":0,"input_tokens":0,"cached_input_tokens":0,"output_tokens":0,"reasoning_tokens":0,"wall_ms":0,"compactions":{"count":0,"tokens_before":0,"tokens_after":0}}"#;

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_deserializes_from_an_object_missing_every_field() {
        let actual: TaskMetrics = serde_json::from_str("{}").unwrap();
        let expected = TaskMetrics::default();

        assert_eq!(actual, expected);
    }

    /// Conversations persisted before `recovery` existed (T1.3) must still
    /// load, per `#[serde(default)]` on the field.
    #[test]
    fn test_task_metrics_without_recovery_field_defaults() {
        let fixture = r#"{"llm_calls":3,"failed_llm_calls":0,"input_tokens":100,"cached_input_tokens":0,"output_tokens":10,"reasoning_tokens":0,"wall_ms":500,"compactions":{"count":0,"tokens_before":0,"tokens_after":0}}"#;

        let actual: TaskMetrics = serde_json::from_str(fixture).unwrap();

        assert_eq!(actual.recovery, RecoveryEvents::default());
        assert_eq!(actual.llm_calls, 3);
    }

    /// A task with no recovery events serializes without the `recovery` key
    /// at all, so old readers of the JSON metrics line are unaffected.
    #[test]
    fn test_recovery_omitted_from_json_when_default() {
        let fixture = TaskMetrics::default().llm_calls(1u64);
        let actual = serde_json::to_string(&fixture).unwrap();

        assert!(!actual.contains("recovery"));
    }

    /// A task with at least one recovery event includes the `recovery` key.
    #[test]
    fn test_recovery_present_in_json_when_nonzero() {
        let mut fixture = TaskMetrics::default();
        fixture.recovery.offload_read = 1;
        let actual = serde_json::to_string(&fixture).unwrap();

        assert!(actual.contains(r#""recovery":{"offload_read":1"#));
    }

    #[test]
    fn test_offload_read_fires_once_per_dump_file() {
        let mut actual = TaskMetrics::default();
        actual.record_dump_file("/tmp/peach_shell_stdout_abc.txt");

        // First read of the dump file recovers the withheld content.
        actual.record_read("/tmp/peach_shell_stdout_abc.txt", 1, 2000);
        // A second read of the same dump file already recovered it; it does
        // not count again.
        actual.record_read("/tmp/peach_shell_stdout_abc.txt", 1, 2000);

        assert_eq!(actual.recovery.offload_read, 1);
    }

    #[test]
    fn test_offload_read_does_not_fire_for_an_unrelated_path() {
        let mut actual = TaskMetrics::default();
        actual.record_dump_file("/tmp/peach_shell_stdout_abc.txt");

        actual.record_read("/home/user/src/main.rs", 1, 50);

        assert_eq!(actual.recovery.offload_read, 0);
    }

    #[test]
    fn test_reread_same_range_fires_on_the_second_identical_read() {
        let mut actual = TaskMetrics::default();
        actual.record_read("/home/user/src/main.rs", 1, 50);
        actual.record_read("/home/user/src/main.rs", 1, 50);

        assert_eq!(actual.recovery.reread_same_range, 1);
    }

    #[test]
    fn test_reread_same_range_does_not_fire_for_a_different_range() {
        let mut actual = TaskMetrics::default();
        actual.record_read("/home/user/src/main.rs", 1, 50);
        actual.record_read("/home/user/src/main.rs", 51, 100);

        assert_eq!(actual.recovery.reread_same_range, 0);
    }

    #[test]
    fn test_reread_same_range_does_not_fire_after_a_write() {
        let mut actual = TaskMetrics::default();
        actual.record_read("/home/user/src/main.rs", 1, 50);
        actual.record_write("/home/user/src/main.rs");
        actual.record_read("/home/user/src/main.rs", 1, 50);

        assert_eq!(actual.recovery.reread_same_range, 0);
    }

    #[test]
    fn test_rerun_same_command_fires_within_the_window() {
        let mut actual = TaskMetrics::default();
        actual.record_shell_command("cargo test");
        actual.llm_calls = 3;
        actual.record_shell_command("cargo test");

        assert_eq!(actual.recovery.rerun_same_command, 1);
    }

    #[test]
    fn test_rerun_same_command_does_not_fire_outside_the_window() {
        let mut actual = TaskMetrics::default();
        actual.record_shell_command("cargo test");
        actual.llm_calls = 6;
        actual.record_shell_command("cargo test");

        assert_eq!(actual.recovery.rerun_same_command, 0);
    }

    #[test]
    fn test_rerun_same_command_does_not_fire_for_a_different_command() {
        let mut actual = TaskMetrics::default();
        actual.record_shell_command("cargo test");
        actual.record_shell_command("cargo build");

        assert_eq!(actual.recovery.rerun_same_command, 0);
    }

    /// The rerun ring buffer is pruned on every call, so it never grows past
    /// the five-call window no matter how many shell commands the task runs.
    #[test]
    fn test_rerun_same_command_history_is_bounded() {
        let mut actual = TaskMetrics::default();
        for i in 0..100 {
            actual.llm_calls = i;
            actual.record_shell_command(&format!("command {i}"));
        }

        assert!(actual.recent_shell_commands.len() <= 5);
    }

    #[test]
    fn test_recovery_events_since_saturates_and_subtracts() {
        let mut baseline = TaskMetrics::default();
        baseline.recovery.offload_read = 2;
        baseline.recovery.rerun_same_command = 1;

        let mut after = baseline.clone();
        after.recovery.offload_read = 5;
        after.recovery.reread_same_range = 3;

        let actual = after.since(&baseline);

        let expected = RecoveryEvents::default()
            .offload_read(3u64)
            .reread_same_range(3u64);

        assert_eq!(actual.recovery, expected);
    }

    #[test]
    fn test_recovery_events_absorb_subagent_adds_counts() {
        let mut child = TaskMetrics::default();
        child.recovery.offload_read = 2;
        child.recovery.rerun_same_command = 1;

        let mut actual = TaskMetrics::default();
        actual.recovery.offload_read = 1;
        actual.absorb_subagent(&child);

        let expected = RecoveryEvents::default()
            .offload_read(3u64)
            .rerun_same_command(1u64);

        assert_eq!(actual.recovery, expected);
    }
}
