//! The standard report (R-HACK-4, HACKATHON.md §18), generated from an
//! evidence bundle (`crate::evidence`).
//!
//! Objective values only (§18: "no subjective claims"). Every number comes
//! from a file in the bundle — token totals from `exec.json`, which the
//! execution layer filled from provider usage (§16), and per-call figures
//! from `telemetry.jsonl` — and nothing is re-run. When an input is missing
//! or unreadable, the section says "not available" and why, rather than
//! reporting zero, which would be a claim.
//!
//! This is the internal schema (`REPORT_SCHEMA_VERSION`); mapping to the
//! organizers' `report.schema.json` belongs in an adapter once it is
//! published (D-020).

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::evidence;
use crate::identity::{HarnessIdentity, REPORT_SCHEMA_VERSION};
use crate::telemetry::event::Envelope;
use crate::telemetry::TelemetryEvent;

/// The generated report file names.
pub const REPORT_JSON: &str = "report.json";
/// See [`REPORT_JSON`].
pub const REPORT_MD: &str = "report.md";

/// The standard report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub schema_version: String,
    pub harness: HarnessIdentity,
    pub outcome: Outcome,
    pub execution: Execution,
    pub tokens: Option<Tokens>,
    pub model_calls: ModelCalls,
    pub context: ContextSection,
    pub tools: Vec<ToolRow>,
    pub error_recovery: ErrorRecovery,
    pub testing: serde_json::Value,
    pub repository: Option<Repository>,
    pub integrity: Option<serde_json::Value>,
    pub timeline: Vec<TimelineEntry>,
    /// Why any section is incomplete.
    pub notes: Vec<String>,
}

/// How the run ended, from `exec.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Outcome {
    pub outcome: Option<String>,
    pub exit_code: Option<i64>,
    pub error: Option<String>,
    pub model: Option<String>,
}

/// Wall time and orchestration shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Execution {
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub wall_ms: Option<u64>,
    /// Distinct agents that emitted events (main agent plus subagents).
    pub agents: Vec<String>,
    pub telemetry_events: usize,
    /// Telemetry lines that could not be parsed, or were dropped at write.
    pub telemetry_lines_lost: u64,
}

/// Token totals from the execution layer, including subagents and failed
/// attempts the provider billed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tokens {
    pub input: u64,
    pub cached_input: u64,
    pub output: u64,
    pub reasoning: u64,
    /// `cached_input / input`; `None` when there was no input.
    pub cache_hit_rate: Option<f64>,
}

/// Model calls, from `exec.json` counts and `model_call` telemetry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelCalls {
    pub calls: Option<u64>,
    pub failed: Option<u64>,
    pub retried_attempts: Option<u64>,
    /// `retry` events in telemetry. Can exceed `retried_attempts`: a run
    /// stopped mid-request (time budget) never folds that request's retries
    /// into the metrics, but each one was logged as it happened.
    pub retry_events: u64,
    /// Input tokens per call, from telemetry.
    pub input_tokens_per_call: Option<Stats>,
    /// Context size before each call, as counted locally (`_estimated`).
    pub context_tokens_estimated: Option<Stats>,
    pub duration_ms: Option<Stats>,
    pub finish_reasons: BTreeMap<String, u64>,
}

/// Minimum, mean and maximum of a series.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Stats {
    pub min: u64,
    pub mean: u64,
    pub max: u64,
}

impl Stats {
    fn of(values: &[u64]) -> Option<Self> {
        let min = *values.iter().min()?;
        let max = *values.iter().max()?;
        let mean = values.iter().sum::<u64>() / values.len() as u64;
        Some(Self { min, mean, max })
    }
}

/// Context management.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextSection {
    pub compactions: u64,
    /// `(messages_before, messages_after)` per compaction, in order.
    pub compaction_messages: Vec<(u64, u64)>,
    pub tokens_reclaimed_estimated: u64,
    /// Estimated tokens by source at the first request (system prompt, tool
    /// definitions, task): the fixed cost every later request repeats.
    #[serde(default)]
    pub prompt_composition: BTreeMap<String, u64>,
    /// Prompt-cache hit rate (`cached / input`) of the model call just before
    /// and just after each compaction, in order (R-CTX-8): how much of the
    /// cached prefix a compaction cost. `None` where a side had no call or
    /// no reported usage.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cache_around_compactions: Vec<(Option<f64>, Option<f64>)>,
}

/// Cache hit rate of one model call, from provider-reported usage.
fn call_cache_rate(call: &crate::telemetry::event::ModelCall) -> Option<f64> {
    let input = call.input_tokens.filter(|input| *input > 0)?;
    Some(call.cached_tokens.unwrap_or(0) as f64 / input as f64)
}

/// One tool's usage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolRow {
    pub name: String,
    pub calls: u64,
    pub errors: u64,
    pub total_duration_ms: u64,
}

/// Failures and what the harness did about them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorRecovery {
    /// Retry attempts by failure reason.
    pub retries_by_reason: BTreeMap<String, u64>,
    /// Retries after an empty completion whose cost is unknown (the provider
    /// reported no usage for it; D-033).
    pub unmetered_empty_completions: u64,
    pub tool_errors: u64,
    pub refused_integrity_actions: u64,
    pub suppressed_prompts: u64,
    /// Times the run moved to a fallback model (`recovery: model_failover`,
    /// D-072). Non-zero means some calls were served by a different model
    /// than the one the run started on, which matters for any per-model
    /// comparison (D-081).
    #[serde(default)]
    pub model_failover_count: u64,
    /// Recovery events by whose failure they answered (`model`, `harness`,
    /// `environment`, `ambiguous`; D-086). Events from logs older than schema
    /// 0.2.0 carry no attribution and are not counted.
    #[serde(default)]
    pub recoveries_by_attribution: BTreeMap<String, u64>,
}

/// What changed in the repository, from `diff.patch`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Repository {
    pub files: Vec<FileChange>,
    pub lines_added: u64,
    pub lines_removed: u64,
}

/// One changed file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileChange {
    pub path: String,
    pub added: u64,
    pub removed: u64,
}

/// One timeline entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimelineEntry {
    pub timestamp: String,
    pub agent: Option<String>,
    pub event: String,
}

fn read_json(dir: &Path, name: &str, notes: &mut Vec<String>) -> Option<serde_json::Value> {
    match std::fs::read_to_string(dir.join(name)) {
        Ok(text) => match serde_json::from_str(&text) {
            Ok(value) => Some(value),
            Err(error) => {
                notes.push(format!("{name}: not available, unparseable: {error}"));
                None
            }
        },
        Err(_) => {
            notes.push(format!("{name}: not available, missing from the bundle"));
            None
        }
    }
}

fn number(value: &serde_json::Value, key: &str) -> Option<u64> {
    value.get(key).and_then(serde_json::Value::as_u64)
}

/// Builds the report from the bundle in `dir`. Never fails: missing inputs
/// become notes.
pub fn build(dir: &Path) -> Report {
    let mut notes = vec![];
    let exec = read_json(dir, evidence::EXEC, &mut notes);
    let integrity = read_json(dir, evidence::INTEGRITY, &mut notes);
    let testing = read_json(dir, evidence::TESTS, &mut notes)
        .unwrap_or_else(|| serde_json::json!({"available": false}));
    let (events, unparseable) = match crate::telemetry::sink::read_jsonl_lenient(&dir.join(evidence::TELEMETRY)) {
        Ok(read) => read,
        Err(_) => {
            notes.push(format!("{}: not available, missing from the bundle", evidence::TELEMETRY));
            (vec![], 0)
        }
    };
    let repository = match std::fs::read_to_string(dir.join(evidence::DIFF)) {
        Ok(diff) => Some(diff_stats(&diff)),
        Err(_) => {
            notes.push(format!("{}: not available, missing from the bundle", evidence::DIFF));
            None
        }
    };

    let metrics = exec.as_ref().and_then(|exec| exec.get("metrics")).cloned();
    let text = |key: &str| {
        exec.as_ref().and_then(|exec| exec.get(key)).and_then(|v| v.as_str()).map(str::to_string)
    };

    let tokens = metrics.as_ref().map(|m| {
        let input = number(m, "input_tokens").unwrap_or(0);
        let cached_input = number(m, "cached_input_tokens").unwrap_or(0);
        Tokens {
            input,
            cached_input,
            output: number(m, "output_tokens").unwrap_or(0),
            reasoning: number(m, "reasoning_tokens").unwrap_or(0),
            cache_hit_rate: (input > 0).then(|| cached_input as f64 / input as f64),
        }
    });

    let mut model_inputs = vec![];
    let mut contexts = vec![];
    let mut durations = vec![];
    let mut finish_reasons = BTreeMap::new();
    let mut tools: BTreeMap<String, ToolRow> = BTreeMap::new();
    let mut recovery = ErrorRecovery {
        retries_by_reason: BTreeMap::new(),
        unmetered_empty_completions: 0,
        tool_errors: 0,
        refused_integrity_actions: 0,
        suppressed_prompts: 0,
        model_failover_count: 0,
        recoveries_by_attribution: BTreeMap::new(),
    };
    let mut context = ContextSection {
        compactions: 0,
        compaction_messages: vec![],
        tokens_reclaimed_estimated: 0,
        prompt_composition: BTreeMap::new(),
        cache_around_compactions: vec![],
    };
    // R-CTX-8: the last call's cache rate, and compactions awaiting their next call.
    let mut last_cache_rate: Option<f64> = None;
    let mut awaiting_next_call: Vec<usize> = vec![];
    let mut agents: Vec<String> = vec![];
    let mut timeline = vec![];
    let mut retry_events = 0u64;
    let (mut started_at, mut ended_at, mut wall_ms, mut dropped) = (None, None, None, 0);

    for Envelope { timestamp, agent_id, event, .. } in &events {
        if let Some(agent) = agent_id
            && !agents.contains(agent)
        {
            agents.push(agent.clone());
        }
        let summary = match event {
            TelemetryEvent::RunStart(_) => {
                started_at = Some(timestamp.clone());
                "run started".to_string()
            }
            TelemetryEvent::RunEnd(end) => {
                ended_at = Some(timestamp.clone());
                wall_ms = Some(end.duration_ms);
                dropped = end.dropped_events;
                format!("run ended: {}", end.outcome)
            }
            TelemetryEvent::ModelCall(call) => {
                let rate = call_cache_rate(call);
                for index in awaiting_next_call.drain(..) {
                    if let Some(entry) = context.cache_around_compactions.get_mut(index) {
                        entry.1 = rate;
                    }
                }
                last_cache_rate = rate;
                model_inputs.extend(call.input_tokens);
                contexts.extend(call.context_tokens_estimated);
                durations.push(call.duration_ms);
                let reason = call.finish_reason.clone().unwrap_or_else(|| "unreported".to_string());
                *finish_reasons.entry(reason.clone()).or_default() += 1;
                format!(
                    "model call: {} in / {} out, {} tool call(s), finish {reason}",
                    call.input_tokens.map_or("?".to_string(), |t| t.to_string()),
                    call.output_tokens.map_or("?".to_string(), |t| t.to_string()),
                    call.tool_call_ids.len()
                )
            }
            TelemetryEvent::ToolCall(call) => {
                let row = tools.entry(call.name.clone()).or_insert_with(|| ToolRow {
                    name: call.name.clone(),
                    calls: 0,
                    errors: 0,
                    total_duration_ms: 0,
                });
                row.calls += 1;
                row.total_duration_ms += call.duration_ms;
                if !call.success {
                    row.errors += 1;
                    recovery.tool_errors += 1;
                }
                format!("tool {}: {}", call.name, if call.success { "ok" } else { "error" })
            }
            TelemetryEvent::Retry(retry) => {
                retry_events += 1;
                *recovery.retries_by_reason.entry(retry_class(&retry.reason)).or_default() += 1;
                if retry.usage_reported == Some(false) {
                    recovery.unmetered_empty_completions += 1;
                }
                format!("retry {} of {}: {}", retry.attempt, retry.operation, retry_class(&retry.reason))
            }
            TelemetryEvent::ContextCompaction(compaction) => {
                awaiting_next_call.push(context.cache_around_compactions.len());
                context.cache_around_compactions.push((last_cache_rate, None));
                context.compactions += 1;
                context
                    .compaction_messages
                    .push((compaction.messages_before as u64, compaction.messages_after as u64));
                context.tokens_reclaimed_estimated += compaction
                    .tokens_before_estimated
                    .unwrap_or(0)
                    .saturating_sub(compaction.tokens_after_estimated.unwrap_or(0));
                format!(
                    "context compacted: {} -> {} messages",
                    compaction.messages_before, compaction.messages_after
                )
            }
            TelemetryEvent::Integrity(integrity) => {
                if integrity.kind == "refused" {
                    recovery.refused_integrity_actions += 1;
                }
                match &integrity.path {
                    Some(path) => format!("integrity {} `{path}`: {}", integrity.kind, integrity.detail),
                    None => format!("integrity {}: {}", integrity.kind, integrity.detail),
                }
            }
            TelemetryEvent::PromptSuppressed(suppressed) => {
                recovery.suppressed_prompts += 1;
                format!("prompt suppressed: {}", suppressed.prompt_kind)
            }
            TelemetryEvent::AgentState(state) => format!("agent {}", state.to),
            TelemetryEvent::Error(error) => format!("error {}: {}", error.kind, error.message),
            TelemetryEvent::Recovery(event) => {
                if event.action == "model_failover" {
                    recovery.model_failover_count += 1;
                }
                if let Some(attribution) = event.attribution {
                    *recovery
                        .recoveries_by_attribution
                        .entry(attribution.as_str().to_string())
                        .or_default() += 1;
                }
                format!("recovery: {}", event.action)
            }
            TelemetryEvent::TestRun(run) => format!("test run: exit {:?}", run.exit_code),
            TelemetryEvent::ContextComposition(composition) => {
                if context.prompt_composition.is_empty() {
                    context.prompt_composition = composition.tokens_by_source_estimated.clone();
                }
                "prompt composition recorded".to_string()
            }
        };
        timeline.push(TimelineEntry { timestamp: timestamp.clone(), agent: agent_id.clone(), event: summary });
    }

    let retried_attempts = metrics.as_ref().and_then(|m| number(m, "retried_llm_calls"));
    if retried_attempts.is_some_and(|attempts| attempts != retry_events) && !events.is_empty() {
        notes.push(format!(
            "retry count differs: {retry_events} retry event(s) in telemetry, {} in exec metrics; \
             retries of a request still in flight when the run stopped reach telemetry only",
            retried_attempts.unwrap_or(0)
        ));
    }

    Report {
        schema_version: REPORT_SCHEMA_VERSION.to_string(),
        harness: HarnessIdentity::current(),
        outcome: Outcome {
            outcome: text("outcome"),
            exit_code: exec.as_ref().and_then(|e| e.get("exit_code")).and_then(|v| v.as_i64()),
            error: text("error"),
            model: text("model"),
        },
        execution: Execution {
            started_at,
            ended_at,
            wall_ms: wall_ms.or_else(|| metrics.as_ref().and_then(|m| number(m, "wall_ms"))),
            agents,
            telemetry_events: events.len(),
            telemetry_lines_lost: unparseable as u64 + dropped,
        },
        tokens,
        model_calls: ModelCalls {
            calls: metrics.as_ref().and_then(|m| number(m, "llm_calls")),
            failed: metrics.as_ref().and_then(|m| number(m, "failed_llm_calls")),
            retried_attempts,
            retry_events,
            input_tokens_per_call: Stats::of(&model_inputs),
            context_tokens_estimated: Stats::of(&contexts),
            duration_ms: Stats::of(&durations),
            finish_reasons,
        },
        context,
        tools: tools.into_values().collect(),
        error_recovery: recovery,
        testing,
        repository,
        integrity,
        timeline,
        notes,
    }
}

/// A stable class for a retry reason: the HTTP status when there is one,
/// else a short label, so the report can count like with like.
fn retry_class(reason: &str) -> String {
    if reason.contains("Empty completion") {
        return "empty_completion".to_string();
    }
    let lower = reason.to_ascii_lowercase();
    if ["connection", "refused", "timed out", "timeout", "dns", "reset by peer"]
        .iter()
        .any(|needle| lower.contains(needle))
    {
        return "transport".to_string();
    }
    let digits: String = reason
        .split(|c: char| !c.is_ascii_digit())
        .find(|part| part.len() == 3 && (part.starts_with('4') || part.starts_with('5')))
        .unwrap_or_default()
        .to_string();
    if digits.is_empty() { "other".to_string() } else { format!("http_{digits}") }
}

/// Per-file added/removed line counts from a unified diff.
fn diff_stats(diff: &str) -> Repository {
    let mut files: Vec<FileChange> = vec![];
    for line in diff.lines() {
        if let Some(rest) = line.strip_prefix("diff --git a/") {
            let path = rest.split(" b/").next().unwrap_or(rest).to_string();
            files.push(FileChange { path, added: 0, removed: 0 });
        } else if let Some(file) = files.last_mut() {
            if line.starts_with('+') && !line.starts_with("+++") {
                file.added += 1;
            } else if line.starts_with('-') && !line.starts_with("---") {
                file.removed += 1;
            }
        }
    }
    Repository {
        lines_added: files.iter().map(|f| f.added).sum(),
        lines_removed: files.iter().map(|f| f.removed).sum(),
        files,
    }
}

fn or_na<T: ToString>(value: Option<T>) -> String {
    value.map_or_else(|| "not available".to_string(), |v| v.to_string())
}

/// Renders the report as markdown.
pub fn render_md(report: &Report) -> String {
    let mut md = String::new();
    let o = &report.outcome;
    let _ = writeln!(md, "# Run report — {} {}\n", report.harness.name, report.harness.version);
    let _ = writeln!(md, "| Outcome | Exit | Model | Wall time |\n|---|---|---|---|");
    let _ = writeln!(
        md,
        "| {} | {} | {} | {} |\n",
        or_na(o.outcome.clone()),
        or_na(o.exit_code),
        or_na(o.model.clone()),
        report.execution.wall_ms.map_or("not available".to_string(), |ms| format!("{:.1} s", ms as f64 / 1000.0))
    );
    if let Some(error) = &o.error {
        let _ = writeln!(md, "Error: `{error}`\n");
    }

    let _ = writeln!(md, "## Tokens (provider-reported)\n");
    match &report.tokens {
        Some(t) => {
            let _ = writeln!(md, "| Input | Cached input | Output | Reasoning | Cache hit rate |\n|---|---|---|---|---|");
            let _ = writeln!(
                md,
                "| {} | {} | {} | {} | {} |\n",
                t.input,
                t.cached_input,
                t.output,
                t.reasoning,
                t.cache_hit_rate.map_or("n/a".to_string(), |r| format!("{:.1}%", r * 100.0))
            );
        }
        None => {
            let _ = writeln!(md, "not available\n");
        }
    }

    let m = &report.model_calls;
    let stats = |s: &Option<Stats>| s.map_or("not available".to_string(), |s| format!("{} / {} / {}", s.min, s.mean, s.max));
    let _ = writeln!(md, "## Model calls\n");
    let _ = writeln!(md, "| Calls | Failed | Retried attempts (metrics) | Retry events (telemetry) | Input tok/call (min/mean/max) | Context tok est. (min/mean/max) | Duration ms (min/mean/max) |\n|---|---|---|---|---|---|---|");
    let _ = writeln!(
        md,
        "| {} | {} | {} | {} | {} | {} | {} |\n",
        or_na(m.calls),
        or_na(m.failed),
        or_na(m.retried_attempts),
        m.retry_events,
        stats(&m.input_tokens_per_call),
        stats(&m.context_tokens_estimated),
        stats(&m.duration_ms)
    );

    let c = &report.context;
    let _ = writeln!(md, "## Context\n");
    let _ = writeln!(
        md,
        "Compactions: {} · tokens reclaimed (estimated): {}{}\n",
        c.compactions,
        c.tokens_reclaimed_estimated,
        if c.compaction_messages.is_empty() {
            String::new()
        } else {
            format!(
                " · messages: {}",
                c.compaction_messages.iter().map(|(b, a)| format!("{b}→{a}")).collect::<Vec<_>>().join(", ")
            )
        }
    );
    if !c.cache_around_compactions.is_empty() {
        let pct = |rate: &Option<f64>| rate.map_or("n/a".to_string(), |r| format!("{:.0}%", r * 100.0));
        let pairs: Vec<String> =
            c.cache_around_compactions.iter().map(|(before, after)| format!("{}→{}", pct(before), pct(after))).collect();
        let _ = writeln!(md, "Cache hit rate around each compaction (call before → call after): {}\n", pairs.join(", "));
    }
    if !c.prompt_composition.is_empty() {
        let total: u64 = c.prompt_composition.values().sum();
        let parts: Vec<String> = c
            .prompt_composition
            .iter()
            .map(|(source, tokens)| format!("{source} {tokens} ({:.0}%)", *tokens as f64 * 100.0 / total.max(1) as f64))
            .collect();
        let _ = writeln!(md, "First request, estimated tokens by source: {}\n", parts.join(" · "));
    }

    let _ = writeln!(md, "## Tools\n");
    if report.tools.is_empty() {
        let _ = writeln!(md, "No tool calls recorded.\n");
    } else {
        let _ = writeln!(md, "| Tool | Calls | Errors | Total ms |\n|---|---|---|---|");
        for tool in &report.tools {
            let _ = writeln!(md, "| {} | {} | {} | {} |", tool.name, tool.calls, tool.errors, tool.total_duration_ms);
        }
        let _ = writeln!(md);
    }

    let r = &report.error_recovery;
    let _ = writeln!(md, "## Error recovery\n");
    let retries = if r.retries_by_reason.is_empty() {
        "none".to_string()
    } else {
        r.retries_by_reason.iter().map(|(k, v)| format!("{k} ×{v}")).collect::<Vec<_>>().join(", ")
    };
    let _ = writeln!(
        md,
        "Retries: {retries} · unmetered empty completions: {} · tool errors: {} · refused test edits: {} · suppressed prompts: {}\n",
        r.unmetered_empty_completions, r.tool_errors, r.refused_integrity_actions, r.suppressed_prompts
    );
    if !r.recoveries_by_attribution.is_empty() {
        let by = r
            .recoveries_by_attribution
            .iter()
            .map(|(k, v)| format!("{k} ×{v}"))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(md, "Recoveries by cause (D-086): {by}\n");
    }
    if r.model_failover_count > 0 {
        let _ = writeln!(
            md,
            "Model failovers: {} — some calls were served by a fallback model (D-072); check `model_call.model`.\n",
            r.model_failover_count
        );
    }

    let _ = writeln!(md, "## Testing (the harness's own run after the agent stopped)\n");
    let _ = writeln!(md, "{}", render_testing(&report.testing));

    let _ = writeln!(md, "## Repository changes\n");
    match &report.repository {
        Some(repo) if repo.files.is_empty() => {
            let _ = writeln!(md, "No changes.\n");
        }
        Some(repo) => {
            let _ = writeln!(md, "{} file(s), +{} −{}\n", repo.files.len(), repo.lines_added, repo.lines_removed);
            let _ = writeln!(md, "| File | + | − |\n|---|---|---|");
            for file in &repo.files {
                let _ = writeln!(md, "| `{}` | {} | {} |", file.path, file.added, file.removed);
            }
            let _ = writeln!(md);
        }
        None => {
            let _ = writeln!(md, "not available\n");
        }
    }
    let _ = writeln!(md, "## Test integrity\n");
    match &report.integrity {
        Some(integrity) => {
            let violations = integrity.get("violations").and_then(|v| v.as_array()).cloned().unwrap_or_default();
            let checked = integrity.get("checked").and_then(|v| v.as_u64()).unwrap_or(0);
            if violations.is_empty() {
                let _ = writeln!(md, "{checked} protected file(s) checked; all unchanged.\n");
            } else {
                let _ = writeln!(md, "{checked} protected file(s) checked; {} violation(s):\n", violations.len());
                for v in violations {
                    let _ = writeln!(
                        md,
                        "- `{}` {}, {}",
                        v.get("path").and_then(|x| x.as_str()).unwrap_or("?"),
                        v.get("kind").and_then(|x| x.as_str()).unwrap_or("?"),
                        if v.get("restored") == Some(&serde_json::Value::Bool(true)) { "restored" } else { "NOT restored" }
                    );
                }
                let _ = writeln!(md);
            }
        }
        None => {
            let _ = writeln!(md, "not available\n");
        }
    }

    let _ = writeln!(md, "## Timeline\n");
    for entry in &report.timeline {
        let _ = writeln!(
            md,
            "- `{}`{} {}",
            entry.timestamp,
            entry.agent.as_ref().map_or(String::new(), |a| format!(" [{a}]")),
            entry.event
        );
    }
    if !report.notes.is_empty() {
        let _ = writeln!(md, "\n## Notes\n");
        for note in &report.notes {
            let _ = writeln!(md, "- {note}");
        }
    }
    md
}

/// The testing section: a table when the harness ran the tests, else why not.
fn render_testing(testing: &serde_json::Value) -> String {
    if testing["ran"] != true {
        return format!(
            "Not run: {}\n",
            testing["reason"].as_str().unwrap_or("not available")
        );
    }
    let field = |key: &str| match &testing[key] {
        serde_json::Value::Null => "n/a".to_string(),
        serde_json::Value::String(text) => text.clone(),
        other => other.to_string(),
    };
    let mut md = String::new();
    let _ = writeln!(md, "| Result | Passed | Failed | Skipped | Exit | Duration ms | Command | Why this command |");
    let _ = writeln!(md, "|---|---|---|---|---|---|---|---|");
    let _ = writeln!(
        md,
        "| {}{} | {} | {} | {} | {} | {} | `{}` | {} |\n",
        field("class"),
        if testing["timed_out"] == true { " (timed out)" } else { "" },
        field("passed"),
        field("failed"),
        field("skipped"),
        field("exit_code"),
        field("duration_ms"),
        field("command"),
        field("source"),
    );
    let tail: Vec<&str> = testing["output_tail"].as_str().unwrap_or_default().lines().collect();
    if !tail.is_empty() {
        let shown = tail.get(tail.len().saturating_sub(12)..).unwrap_or_default();
        let _ = writeln!(md, "Last {} line(s) of output:\n\n```\n{}\n```\n", shown.len(), shown.join("\n"));
    }
    md
}

/// Builds the report for the bundle in `dir` and writes `report.json` and
/// `report.md` into it.
///
/// # Errors
/// Returns an error when either file cannot be written.
pub fn generate(dir: &Path) -> anyhow::Result<Report> {
    let report = build(dir);
    std::fs::write(dir.join(REPORT_JSON), serde_json::to_string_pretty(&report)?)?;
    std::fs::write(dir.join(REPORT_MD), render_md(&report))?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    const TELEMETRY_FIXTURE: &str = include_str!("../fixtures/evidence_min/telemetry.jsonl");

    fn fixture_bundle() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/evidence_min");
        for name in ["exec.json", "integrity.json", "tests.json", "diff.patch"] {
            std::fs::copy(root.join(name), dir.path().join(name)).unwrap();
        }
        std::fs::write(dir.path().join(evidence::TELEMETRY), TELEMETRY_FIXTURE).unwrap();
        dir
    }

    #[test]
    fn test_report_from_a_fixture_bundle() {
        let fixture = fixture_bundle();

        let actual = build(fixture.path());

        // Version fields vary by build; the rest is golden.
        let mut json = serde_json::to_value(&actual).unwrap();
        json["harness"] = serde_json::json!("<identity>");
        insta::assert_json_snapshot!("report_json", json);
        let mut normalized = actual.clone();
        normalized.harness = HarnessIdentity { name: "peach-ice-tea".into(), version: "0".into(), build: None };
        insta::assert_snapshot!("report_md", render_md(&normalized));
    }

    #[test]
    fn test_a_partial_bundle_reports_what_is_missing_not_zero() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("diff.patch"), "").unwrap();

        let actual = build(dir.path());

        assert_eq!(actual.tokens, None);
        assert_eq!(actual.model_calls.calls, None);
        assert_eq!(actual.repository.map(|r| r.files.len()), Some(0));
        assert_eq!(
            actual.notes,
            vec![
                "exec.json: not available, missing from the bundle",
                "integrity.json: not available, missing from the bundle",
                "tests.json: not available, missing from the bundle",
                "telemetry.jsonl: not available, missing from the bundle",
            ]
        );
        assert!(render_md(&build(dir.path())).contains("| not available | not available |"));
    }

    #[test]
    fn test_retries_only_in_telemetry_are_shown_and_explained() {
        let fixture = fixture_bundle();
        let exec = std::fs::read_to_string(fixture.path().join("exec.json"))
            .unwrap()
            .replace(r#""retried_llm_calls": 2"#, r#""retried_llm_calls": 0"#);
        std::fs::write(fixture.path().join("exec.json"), exec).unwrap();

        let actual = build(fixture.path());

        assert_eq!((actual.model_calls.retried_attempts, actual.model_calls.retry_events), (Some(0), 2));
        assert_eq!(
            actual.notes,
            vec![
                "retry count differs: 2 retry event(s) in telemetry, 0 in exec metrics; \
                 retries of a request still in flight when the run stopped reach telemetry only"
            ]
        );
    }

    #[test]
    fn test_a_harness_test_run_renders_as_a_table_with_a_short_tail() {
        let fixture = serde_json::json!({
            "ran": true, "command": "python3 -m unittest", "source": "explicit", "exit_code": 1,
            "timed_out": false, "duration_ms": 110, "class": "test_assertion",
            "passed": 3, "failed": 1, "skipped": 0,
            "output_tail": (1..=30).map(|n| format!("line {n}")).collect::<Vec<_>>().join("\n"),
        });

        let actual = render_testing(&fixture);

        assert!(actual.contains("| test_assertion | 3 | 1 | 0 | 1 | 110 | `python3 -m unittest` | explicit |"));
        assert!(actual.contains("Last 12 line(s)") && actual.contains("line 30") && !actual.contains("line 18\n"));
    }

    #[test]
    fn test_recoveries_are_tallied_by_whose_failure_they_answered() {
        let fixture = fixture_bundle();
        let recovery = |action: &str, attribution: Option<&str>| {
            let mut event = serde_json::json!({"type": "recovery", "action": action, "trigger": "t"});
            if let Some(attribution) = attribution {
                event["attribution"] = attribution.into();
            }
            let mut line: serde_json::Value =
                serde_json::from_str(TELEMETRY_FIXTURE.lines().last().unwrap()).unwrap();
            line["event"] = event;
            line.to_string()
        };
        let telemetry = [
            TELEMETRY_FIXTURE.trim_end().to_string(),
            recovery("model_failover", Some("harness")),
            recovery("rerun_same_command", Some("model")),
            recovery("reread_same_range", Some("model")),
            recovery("recovery_hint", None),
        ]
        .join("\n");
        std::fs::write(fixture.path().join(evidence::TELEMETRY), telemetry).unwrap();

        let actual = build(fixture.path()).error_recovery.recoveries_by_attribution;

        let expected = BTreeMap::from([("harness".to_string(), 1), ("model".to_string(), 2)]);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_retry_reasons_are_classed() {
        let actual: Vec<String> = [
            "Retryable(Empty completion received - no content, tool calls, or valid finish reason)",
            "InvalidStatusCode(429)",
            "http status 503 Service Unavailable",
            "Connection refused (os error 61)",
            "something unexpected",
        ]
        .iter()
        .map(|r| retry_class(r))
        .collect();

        assert_eq!(actual, vec!["empty_completion", "http_429", "http_503", "transport", "other"]);
    }

    #[test]
    fn test_diff_stats_count_lines_per_file() {
        let diff = "diff --git a/math.py b/math.py\n--- a/math.py\n+++ b/math.py\n@@ -1,2 +1,2 @@\n def add(a, b):\n-    return a - b\n+    return a + b\ndiff --git a/new.py b/new.py\n--- /dev/null\n+++ b/new.py\n@@ -0,0 +1 @@\n+x = 1\n";

        let actual = diff_stats(diff);

        assert_eq!(
            actual,
            Repository {
                files: vec![
                    FileChange { path: "math.py".into(), added: 1, removed: 1 },
                    FileChange { path: "new.py".into(), added: 1, removed: 0 },
                ],
                lines_added: 2,
                lines_removed: 1,
            }
        );
    }
}
