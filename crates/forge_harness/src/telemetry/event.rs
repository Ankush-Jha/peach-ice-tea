//! The telemetry contract: one envelope type, one event enum.
//!
//! `HACKATHON.md` §15 lists what must be captured (model calls, context,
//! tools, agent state, testing, execution) and §16 draws a hard line: an
//! objective value such as a token count must come from the execution layer
//! (provider `Usage`, an exit code, a parsed test summary), never from a
//! model-generated claim. Where a number here is instead computed locally —
//! a client-side token estimate ahead of a call, or a size used to decide
//! whether to compact — its field name says so with an `_estimated` suffix,
//! so nobody downstream mistakes it for a measurement (`ALIGNMENT.md`
//! R-HACK-3, `DECISIONS.md` D-020).
//!
//! This module defines the schema only. Nothing here writes anything —
//! that's `sink.rs` — and nothing here is wired into the orchestrator yet —
//! that's a later piece's job, per the task boundary for TH.4.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::identity::HarnessIdentity;

/// Text that may have been shortened before it was logged.
///
/// Any field that could carry an unbounded amount of model- or tool-supplied
/// text (tool arguments, tool result summaries) uses this type instead of a
/// bare `String`, so a truncation decision is always visible in the event
/// itself rather than silently losing information (`CLAUDE.md` principle 4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Truncated {
    /// The text as logged; equal to the source when `truncated` is `false`.
    pub text: String,
    /// Whether `text` is a prefix of something longer.
    pub truncated: bool,
    /// Length, in characters, of the untruncated source text.
    pub original_chars: usize,
}

impl Truncated {
    /// Records `text` verbatim, with no limit applied.
    ///
    /// Used where a sink is not involved (tests, and any caller that wants
    /// to record a value it already knows is bounded).
    pub fn whole(text: impl Into<String>) -> Self {
        let text = text.into();
        let original_chars = text.chars().count();
        Self { text, truncated: false, original_chars }
    }

    /// Records `text`, cutting it to at most `max_chars` characters.
    ///
    /// `max_chars` is a character count, not a byte count, so the cut point
    /// can never land inside a multi-byte character.
    pub fn capped(text: &str, max_chars: usize) -> Self {
        let original_chars = text.chars().count();
        if original_chars <= max_chars {
            return Self { text: text.to_string(), truncated: false, original_chars };
        }
        let text: String = text.chars().take(max_chars).collect();
        Self { text, truncated: true, original_chars }
    }
}

/// The envelope every event is wrapped in before it is written.
///
/// Carries everything that is true of the event stream as a whole — which
/// schema, which harness, which run, monotonic ordering, when — so
/// individual event variants only carry what is specific to them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Envelope {
    /// Identifies this as a Peach Ice Tea internal telemetry event, ahead of
    /// the organizer adapter (`organizer_adapter.rs`, D-020).
    pub schema_id: String,
    /// Version of the internal schema, from `crate::identity`.
    pub schema_version: String,
    /// Which harness build produced this event (`DECISIONS.md` D-023).
    pub harness: HarnessIdentity,
    /// Identifies one execution of the harness; shared by every event in a
    /// run.
    pub run_id: String,
    /// Strictly increasing within a run, starting at zero. Ordering
    /// survives even if the underlying clock does not have millisecond
    /// resolution or goes backwards.
    pub seq: u64,
    /// RFC 3339 timestamp with millisecond precision, e.g.
    /// `2026-09-23T10:15:30.123Z`.
    pub timestamp: String,
    /// Which conversation this event belongs to, when the run has more than
    /// one (subagents, resumed sessions).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<String>,
    /// Which agent (main loop or subagent) emitted this event.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    /// The event payload.
    pub event: TelemetryEvent,
}

/// Everything the telemetry stream can report.
///
/// Serialised with a `type` tag holding the `snake_case` variant name, so a
/// consumer that has not seen a new variant yet can still parse the ones it
/// knows (`serde`'s internally-tagged representation), matching the
/// convention already used for tagged enums in this codebase
/// (`forge_domain::node::NodeData`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TelemetryEvent {
    /// The run began. HACKATHON.md §15 "Execution: start".
    RunStart(RunStart),
    /// The run ended. HACKATHON.md §15 "Execution: end, total duration".
    RunEnd(RunEnd),
    /// One request/response cycle with the model. HACKATHON.md §15 "Model".
    ModelCall(ModelCall),
    /// One tool invocation and its result. HACKATHON.md §15 "Tools".
    ToolCall(ToolCall),
    /// Context was compacted. HACKATHON.md §15 "Context: compression".
    ContextCompaction(ContextCompaction),
    /// A snapshot of what is currently occupying the context.
    /// HACKATHON.md §15 "Context: composition".
    ContextComposition(ContextComposition),
    /// The agent (main loop or a subagent) changed state.
    /// HACKATHON.md §15 "Agent: state, orchestration transitions".
    AgentState(AgentState),
    /// An operation was retried. HACKATHON.md §15 "Agent: retries".
    Retry(Retry),
    /// Something failed. HACKATHON.md §15 "Agent: errors".
    Error(Error),
    /// The harness took an action in response to a failure.
    /// HACKATHON.md §15 "Agent: recovery actions".
    Recovery(Recovery),
    /// A test command was executed. HACKATHON.md §15 "Testing".
    TestRun(TestRun),
    /// A test-integrity check or restore (R-HACK-2) took place.
    Integrity(Integrity),
    /// An interactive prompt was suppressed because the run is unattended.
    ///
    /// Not listed verbatim in §15, but a prompt that would otherwise have
    /// blocked forever (`DECISIONS.md` D-022) is exactly the kind of thing
    /// §23 ("can the harness identify failure ... without human
    /// intervention?") and §16 ask to be a loud, recorded fact rather than a
    /// silent behaviour change.
    PromptSuppressed(PromptSuppressed),
}

/// The run began.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunStart {
    /// Free-form identifier for the task being attempted, when known
    /// (e.g. an issue id from the evaluation harness).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    /// Repository root the run operates on.
    pub repo_root: String,
}

/// The run ended.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunEnd {
    /// How the run concluded, e.g. `"completed"`, `"error"`, `"interrupted"`.
    pub outcome: String,
    /// Wall-clock duration of the whole run.
    pub duration_ms: u64,
    /// Events this sink failed to write over the course of the run
    /// (fail-open: the run continues regardless; this is the visible trace
    /// of that fact).
    pub dropped_events: u64,
}

/// One request/response cycle with the model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelCall {
    /// Provider- or harness-assigned identifier for this call.
    pub call_id: String,
    /// RFC 3339 timestamp for when the request was sent.
    pub started_at: String,
    /// RFC 3339 timestamp for when the response (or failure) was received.
    pub ended_at: String,
    /// `ended_at - started_at`, precomputed so a consumer never has to parse
    /// two timestamps to get a duration.
    pub duration_ms: u64,
    /// Model identifier as sent to the provider.
    pub model: String,
    /// Input tokens, from the provider's usage report. `None` when the
    /// provider did not return one (e.g. the call failed before a response).
    pub input_tokens: Option<u64>,
    /// Output tokens, from the provider's usage report.
    pub output_tokens: Option<u64>,
    /// Total tokens, from the provider's usage report.
    pub total_tokens: Option<u64>,
    /// Cached input tokens, from the provider's usage report, when the
    /// provider supports prompt caching.
    pub cached_tokens: Option<u64>,
    /// Reasoning/thinking tokens, from the provider's usage report
    /// (Gemini's `thoughtsTokenCount`, R-HACK-6).
    pub reasoning_tokens: Option<u64>,
    /// Size of the context sent with this call, in tokens, as counted
    /// locally by the harness ahead of the request. Named `_estimated`
    /// because it is not the provider's measurement (§16): the provider's
    /// own count is `input_tokens`.
    pub context_tokens_estimated: Option<u64>,
    /// Number of messages in the context sent with this call. A count, not
    /// an estimate.
    pub context_messages: usize,
    /// Why the model stopped generating (`"stop"`, `"tool_calls"`,
    /// `"length"`, ...), when the provider reports one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
    /// `call_id` of every [`ToolCall`] the model requested in this response,
    /// in the order requested. A bare count is not a safe way to correlate
    /// tool calls back to the response that produced them once tool calls
    /// can run in parallel (T2.1); carrying the ids instead makes that link
    /// explicit, and the count is simply `tool_call_ids.len()`.
    pub tool_call_ids: Vec<String>,
}

/// One tool invocation and its result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolCall {
    /// Identifier correlating the call with its result.
    pub call_id: String,
    /// Tool name as it appears in the catalog.
    pub name: String,
    /// Arguments the model supplied, truncated per the sink's configured
    /// limit.
    pub arguments: Truncated,
    /// Whether the tool reported success.
    pub success: bool,
    /// Wall-clock duration of the call.
    pub duration_ms: u64,
    /// Length, in characters, of the full (untruncated) result. Independent
    /// of any truncation applied to `result_summary`, so the true size is
    /// always on record even when the summary is not.
    pub result_size_chars: usize,
    /// A (possibly truncated) view of the result, per the sink's configured
    /// limit.
    pub result_summary: Truncated,
    /// Handle the result was stored under, when the result was large enough
    /// to be handed off rather than returned inline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handle: Option<String>,
    /// `call_id` of the [`ModelCall`] whose response requested this tool
    /// call, when known. Adjacent `seq` is not a safe way to infer this
    /// once tool calls can run in parallel (T2.1), so the link is carried
    /// explicitly rather than left to be reconstructed after the fact.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin_call_id: Option<String>,
}

/// Context was compacted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextCompaction {
    /// Number of messages in context immediately before compaction.
    pub messages_before: usize,
    /// Number of messages in context immediately after compaction.
    pub messages_after: usize,
    /// Token size of the context immediately before compaction, as counted
    /// locally. Named `_estimated` per §16: no provider measurement exists
    /// for a context that was never sent as-is.
    pub tokens_before_estimated: Option<u64>,
    /// Token size of the context immediately after compaction, as counted
    /// locally.
    pub tokens_after_estimated: Option<u64>,
    /// What kinds of content survived compaction (e.g. `"reasoning_chain"`,
    /// `"todo_list"`, `"recent_tool_result"`). §15 asks for "retained
    /// state", and `CLAUDE.md` principle 7 requires reasoning-chain
    /// preservation not to regress; neither claim is checkable from
    /// telemetry without naming what was actually kept, so this is a list
    /// of freeform kind labels rather than a bare "compaction happened"
    /// fact. Empty when the compactor emitting this event does not yet
    /// report retained-content kinds — not a claim that nothing survived.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub retained_content_kinds: Vec<String>,
}

/// A snapshot of what is currently occupying the context.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextComposition {
    /// Total number of messages in context at the time of the snapshot.
    pub total_messages: usize,
    /// Locally counted token share per message role (`"system"`, `"user"`,
    /// `"assistant"`, `"tool"`, ...). Named `_estimated` per §16.
    pub tokens_by_role_estimated: BTreeMap<String, u64>,
    /// Locally counted token share per content source (e.g. `"system_prompt"`,
    /// `"tool_result"`, `"summary"`). Named `_estimated` per §16.
    pub tokens_by_source_estimated: BTreeMap<String, u64>,
}

/// The agent (main loop or a subagent) changed state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentState {
    /// State transitioned from, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// State transitioned to.
    pub to: String,
    /// Why the transition happened, when relevant (e.g. a termination
    /// reason).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Orchestrator loop iteration this transition happened on, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iteration: Option<u64>,
}

/// An operation was retried.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Retry {
    /// What is being retried (e.g. `"model_call"`, `"tool_call:shell"`).
    pub operation: String,
    /// Which attempt this is, starting at 1 for the first retry.
    pub attempt: u32,
    /// The configured retry ceiling, when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_attempts: Option<u32>,
    /// Why the previous attempt failed.
    pub reason: String,
    /// `call_id` of the [`ModelCall`] or [`ToolCall`] being retried, when
    /// known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin_call_id: Option<String>,
    /// For a failed attempt that produced a response (an empty completion),
    /// whether the provider reported usage for it: `true` with the counts
    /// below, `false` when it reported none — which means the attempt's cost
    /// is unknown, not zero. Absent for failures with no response body to
    /// bill (HTTP 429/5xx, transport errors).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage_reported: Option<bool>,
    /// Provider-reported input tokens for the failed attempt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u64>,
    /// Provider-reported output tokens (thinking included) for the failed
    /// attempt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u64>,
    /// Provider-reported reasoning tokens for the failed attempt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<u64>,
}

/// Something failed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Error {
    /// A short, stable classification (e.g. `"tool_error"`, `"compile"`,
    /// `"environment"`), not a full-sentence description.
    pub kind: String,
    /// Human-readable detail.
    pub message: String,
    /// Whether the harness has a recovery path for this error, as opposed
    /// to it ending the run.
    pub recoverable: bool,
    /// Where the error originated (e.g. `"tool:shell"`, `"model"`), when
    /// known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// `call_id` of the [`ModelCall`] or [`ToolCall`] this error originated
    /// from, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin_call_id: Option<String>,
}

/// The harness took an action in response to a failure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recovery {
    /// The action taken (e.g. `"recovery_hint_injected"`, `"file_restored"`).
    pub action: String,
    /// What triggered the recovery (an error kind, a failed test run, ...).
    pub trigger: String,
    /// The result of taking the action, when known at the time it is
    /// logged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<String>,
    /// `call_id` of the [`ModelCall`] or [`ToolCall`] that triggered this
    /// recovery, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin_call_id: Option<String>,
}

/// A test command was executed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestRun {
    /// The command that was run.
    pub command: String,
    /// Process exit code, when the process ran to completion.
    pub exit_code: Option<i32>,
    /// Passing test count, parsed from the command's own output. `None`
    /// when the harness could not parse a count for this runner.
    pub passed: Option<u64>,
    /// Failing test count, parsed from the command's own output.
    pub failed: Option<u64>,
    /// Skipped test count, parsed from the command's own output.
    pub skipped: Option<u64>,
    /// Wall-clock duration of the command.
    pub duration_ms: u64,
    /// What kind of result this was (`passed`, `test_assertion`, `compile`,
    /// `environment`, `timeout`, `unknown`), from `verify::classify`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_class: Option<String>,
    /// Who ran it: `agent` (a shell call the model made) or `harness_final`
    /// (the harness's own run after the agent stopped).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
}

/// A test-integrity check or restore took place (R-HACK-2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Integrity {
    /// A short, stable classification (e.g. `"refused"`, `"violation"`,
    /// `"restored"`).
    pub kind: String,
    /// The path involved, when the event concerns one specific path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Human-readable detail.
    pub detail: String,
}

/// An interactive prompt was suppressed because the run is unattended.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromptSuppressed {
    /// What kind of prompt this would have been (e.g. `"tool_permission"`,
    /// `"shell_confirmation"`).
    pub prompt_kind: String,
    /// What would have been asked.
    pub detail: String,
    /// What the harness did instead of asking, when it took a default
    /// action rather than simply refusing (e.g. `"denied"`,
    /// `"auto_approved"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_action: Option<String>,
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_truncated_whole_keeps_text_untruncated() {
        let actual = Truncated::whole("hello");

        let expected =
            Truncated { text: "hello".to_string(), truncated: false, original_chars: 5 };
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_truncated_capped_leaves_short_text_alone() {
        let actual = Truncated::capped("hello", 10);

        let expected =
            Truncated { text: "hello".to_string(), truncated: false, original_chars: 5 };
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_truncated_capped_cuts_long_text_and_flags_it() {
        let actual = Truncated::capped("hello world", 5);

        let expected =
            Truncated { text: "hello".to_string(), truncated: true, original_chars: 11 };
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_truncated_capped_cuts_on_char_boundaries() {
        // "café" is 4 chars but 5 bytes (é is 2 bytes in UTF-8); a byte-based
        // cut at 3 would split the é and panic or corrupt the string.
        let actual = Truncated::capped("café", 3);

        assert_eq!(actual.text, "caf");
        assert!(actual.truncated);
        assert_eq!(actual.original_chars, 4);
    }

    #[test]
    fn test_tool_call_carries_an_optional_origin_call_id() {
        let fixture = ToolCall {
            call_id: "call-2".to_string(),
            name: "shell".to_string(),
            arguments: Truncated::whole("ls"),
            success: true,
            duration_ms: 5,
            result_size_chars: 3,
            result_summary: Truncated::whole("out"),
            handle: None,
            origin_call_id: Some("model-call-1".to_string()),
        };

        let actual = serde_json::to_value(&fixture).unwrap();

        assert_eq!(actual["origin_call_id"], "model-call-1");
    }

    #[test]
    fn test_origin_call_id_is_omitted_from_json_when_absent() {
        let fixture = Retry {
            operation: "model_call".to_string(),
            attempt: 1,
            max_attempts: Some(3),
            reason: "timeout".to_string(),
            origin_call_id: None,
            usage_reported: None,
            input_tokens: None,
            output_tokens: None,
            reasoning_tokens: None,
        };

        let actual = serde_json::to_value(&fixture).unwrap();

        assert!(actual.get("origin_call_id").is_none());
        assert!(actual.get("usage_reported").is_none());
    }

    #[test]
    fn test_error_and_recovery_also_carry_an_origin_call_id() {
        let error = Error {
            kind: "tool_error".to_string(),
            message: "command not found".to_string(),
            recoverable: true,
            source: Some("tool:shell".to_string()),
            origin_call_id: Some("call-3".to_string()),
        };
        let recovery = Recovery {
            action: "recovery_hint_injected".to_string(),
            trigger: "tool_error".to_string(),
            outcome: None,
            origin_call_id: Some("call-3".to_string()),
        };

        assert_eq!(serde_json::to_value(&error).unwrap()["origin_call_id"], "call-3");
        assert_eq!(serde_json::to_value(&recovery).unwrap()["origin_call_id"], "call-3");
    }

    #[test]
    fn test_model_call_carries_the_list_of_tool_call_ids_it_requested() {
        let fixture = ModelCall {
            call_id: "model-call-1".to_string(),
            started_at: "2026-09-23T10:15:30.000Z".to_string(),
            ended_at: "2026-09-23T10:15:31.000Z".to_string(),
            duration_ms: 1_000,
            model: "gemini-3.8-high".to_string(),
            input_tokens: Some(100),
            output_tokens: Some(20),
            total_tokens: Some(120),
            cached_tokens: None,
            reasoning_tokens: None,
            context_tokens_estimated: Some(90),
            context_messages: 4,
            finish_reason: Some("tool_calls".to_string()),
            tool_call_ids: vec!["call-1".to_string(), "call-2".to_string()],
        };

        let actual = serde_json::to_value(&fixture).unwrap();

        assert_eq!(actual["tool_call_ids"], serde_json::json!(["call-1", "call-2"]));
    }

    #[test]
    fn test_context_compaction_names_what_content_was_retained() {
        let fixture = ContextCompaction {
            messages_before: 40,
            messages_after: 10,
            tokens_before_estimated: Some(20_000),
            tokens_after_estimated: Some(4_000),
            retained_content_kinds: vec!["reasoning_chain".to_string(), "todo_list".to_string()],
        };

        let actual = serde_json::to_value(&fixture).unwrap();

        assert_eq!(
            actual["retained_content_kinds"],
            serde_json::json!(["reasoning_chain", "todo_list"])
        );
    }

    #[test]
    fn test_context_compaction_omits_retained_content_kinds_when_empty() {
        let fixture = ContextCompaction {
            messages_before: 40,
            messages_after: 10,
            tokens_before_estimated: Some(20_000),
            tokens_after_estimated: Some(4_000),
            retained_content_kinds: Vec::new(),
        };

        let actual = serde_json::to_value(&fixture).unwrap();

        assert!(actual.get("retained_content_kinds").is_none());
    }

    #[test]
    fn test_telemetry_event_serializes_with_a_type_tag() {
        let fixture = TelemetryEvent::AgentState(AgentState {
            from: Some("planning".to_string()),
            to: "executing".to_string(),
            reason: None,
            iteration: Some(3),
        });

        let actual = serde_json::to_value(&fixture).unwrap();

        assert_eq!(actual["type"], "agent_state");
        assert_eq!(actual["to"], "executing");
        assert_eq!(actual["from"], "planning");
        assert!(actual.get("reason").is_none());
    }

    #[test]
    fn test_envelope_round_trips_through_json() {
        let fixture = Envelope {
            schema_id: "peach-ice-tea.telemetry".to_string(),
            schema_version: "0.1.0".to_string(),
            harness: HarnessIdentity::current(),
            run_id: "run-1".to_string(),
            seq: 0,
            timestamp: "2026-09-23T10:15:30.123Z".to_string(),
            conversation_id: Some("conv-1".to_string()),
            agent_id: None,
            event: TelemetryEvent::RunStart(RunStart {
                task_id: Some("issue-42".to_string()),
                repo_root: "/repo".to_string(),
            }),
        };

        let json = serde_json::to_string(&fixture).unwrap();
        let actual: Envelope = serde_json::from_str(&json).unwrap();

        assert_eq!(actual, fixture);
    }

    #[test]
    fn test_envelope_rejects_unknown_fields() {
        let json = r#"{
            "schema_id": "peach-ice-tea.telemetry",
            "schema_version": "0.1.0",
            "harness": {"name": "peach-ice-tea", "version": "0.1.0"},
            "run_id": "run-1",
            "seq": 0,
            "timestamp": "2026-09-23T10:15:30.123Z",
            "event": {"type": "run_start", "repo_root": "/repo"},
            "unexpected_field": true
        }"#;

        let actual: Result<Envelope, _> = serde_json::from_str(json);

        assert!(actual.is_err());
    }
}
