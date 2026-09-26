//! Where telemetry events go.
//!
//! Telemetry is optional infrastructure: a run that cannot write its
//! telemetry log must still complete the task (`CLAUDE.md` principle 5,
//! "fail open"). Every path through `Sink::emit` therefore ends in either a
//! successful write or a swallowed-and-logged failure — never a propagated
//! error, and never a panic.

use std::collections::VecDeque;
use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use crate::identity::{HarnessIdentity, TELEMETRY_SCHEMA_VERSION};
use crate::telemetry::event::{Envelope, TelemetryEvent, Truncated};

/// Identifies the internal telemetry schema in every envelope
/// (`organizer_adapter.rs` maps to the organizers' schema separately;
/// `DECISIONS.md` D-020).
pub const SCHEMA_ID: &str = "peach-ice-tea.telemetry";

/// Configurable ceilings on how much text one event may carry.
///
/// Applied on write, on top of whatever the caller already put in a
/// [`Truncated`] field, so one enormous tool result cannot make the log
/// unusable regardless of what produced it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SinkLimits {
    /// Maximum characters kept for a tool call's `arguments`.
    pub max_argument_chars: usize,
    /// Maximum characters kept for a tool call's `result_summary`.
    pub max_result_chars: usize,
}

impl Default for SinkLimits {
    fn default() -> Self {
        Self { max_argument_chars: 2_000, max_result_chars: 4_000 }
    }
}

/// Where telemetry events are written.
///
/// `NoOp` is the default and costs nothing per event; nothing in an
/// interactive session or an existing test observes a difference. `Jsonl`
/// appends one JSON object per line to a file.
pub enum Sink {
    /// Discards every event. The default until a caller opts in.
    NoOp,
    /// Appends one JSON line per event to a file.
    Jsonl(JsonlSink),
}

impl Sink {
    /// A sink that discards everything.
    pub fn noop() -> Self {
        Sink::NoOp
    }

    /// A sink that appends JSON lines to `path`.
    ///
    /// Never fails: opening the underlying file is deferred to the first
    /// `emit` call, and a failure there is swallowed per this module's
    /// fail-open contract. This means a bad path (e.g. a directory that
    /// does not exist) is only ever visible as dropped events, never as a
    /// construction-time error the caller would have to handle.
    pub fn jsonl(path: impl Into<PathBuf>, run_id: impl Into<String>, limits: SinkLimits) -> Self {
        Sink::Jsonl(JsonlSink::new(path.into(), run_id.into(), limits))
    }

    /// Records one event, tagging it with `conversation_id`/`agent_id` when
    /// given. A no-op when this is [`Sink::NoOp`].
    pub fn emit(
        &self,
        event: TelemetryEvent,
        conversation_id: Option<String>,
        agent_id: Option<String>,
    ) {
        match self {
            Sink::NoOp => {}
            Sink::Jsonl(sink) => sink.emit(event, conversation_id, agent_id),
        }
    }

    /// Events dropped because a write failed. Always `0` for [`Sink::NoOp`].
    pub fn dropped_events(&self) -> u64 {
        match self {
            Sink::NoOp => 0,
            Sink::Jsonl(sink) => sink.dropped.load(Ordering::Relaxed),
        }
    }
}

/// Internal state for the JSONL sink.
///
/// The file handle is opened lazily, on the first successful write, so
/// construction can never fail: a bad path only ever shows up as dropped
/// events, consistent with fail-open.
pub struct JsonlSink {
    path: PathBuf,
    run_id: String,
    limits: SinkLimits,
    seq: AtomicU64,
    dropped: AtomicU64,
    writer: Mutex<Option<BufWriter<std::fs::File>>>,
}

impl JsonlSink {
    fn new(path: PathBuf, run_id: String, limits: SinkLimits) -> Self {
        Self {
            path,
            run_id,
            limits,
            seq: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
            writer: Mutex::new(None),
        }
    }

    fn emit(&self, event: TelemetryEvent, conversation_id: Option<String>, agent_id: Option<String>) {
        // Allocate the sequence number unconditionally: ordering must stay
        // monotonic even for the events that end up dropped, so a consumer
        // can tell from a gap that something was lost rather than being
        // silently misled about ordering.
        let seq = self.seq.fetch_add(1, Ordering::SeqCst);
        let event = enforce_limits(event, self.limits);
        let envelope = Envelope {
            schema_id: SCHEMA_ID.to_string(),
            schema_version: TELEMETRY_SCHEMA_VERSION.to_string(),
            harness: HarnessIdentity::current(),
            run_id: self.run_id.clone(),
            seq,
            timestamp: now_rfc3339_millis(),
            conversation_id,
            agent_id,
            event,
        };

        if let Err(error) = self.write_line(&envelope) {
            self.dropped.fetch_add(1, Ordering::Relaxed);
            tracing::warn!(
                error = %error,
                path = %self.path.display(),
                "telemetry: dropping event, sink write failed (fail-open)"
            );
        }
    }

    fn write_line(&self, envelope: &Envelope) -> anyhow::Result<()> {
        let line = serde_json::to_string(envelope)?;

        let mut guard =
            self.writer.lock().map_err(|_| anyhow::anyhow!("telemetry sink lock poisoned"))?;
        if guard.is_none() {
            let file = OpenOptions::new().create(true).append(true).open(&self.path)?;
            *guard = Some(BufWriter::new(file));
        }
        // `is_some()` was just established above; this cannot panic.
        let writer = guard.as_mut().expect("sink writer initialised above");
        writer.write_all(line.as_bytes())?;
        writer.write_all(b"\n")?;
        writer.flush()?;
        Ok(())
    }
}

/// Re-applies this sink's size limits to a [`TelemetryEvent`], regardless of
/// what the caller already put in its [`Truncated`] fields.
fn enforce_limits(event: TelemetryEvent, limits: SinkLimits) -> TelemetryEvent {
    match event {
        TelemetryEvent::ToolCall(mut tool_call) => {
            tool_call.arguments = cap(tool_call.arguments, limits.max_argument_chars);
            tool_call.result_summary = cap(tool_call.result_summary, limits.max_result_chars);
            TelemetryEvent::ToolCall(tool_call)
        }
        other => other,
    }
}

/// Caps `existing` to `max_chars`, preserving the true original size even if
/// `existing` was already truncated to something shorter by its caller.
fn cap(existing: Truncated, max_chars: usize) -> Truncated {
    if existing.text.chars().count() <= max_chars {
        return existing;
    }
    let mut capped = Truncated::capped(&existing.text, max_chars);
    capped.original_chars = capped.original_chars.max(existing.original_chars);
    capped
}

/// The current time as an RFC 3339 timestamp with millisecond precision.
fn now_rfc3339_millis() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

/// Reads a JSONL telemetry file back into its events, in file order.
///
/// Used by tests and, later, by the report generator (R-HACK-4). Skips
/// nothing and fixes nothing: a malformed line is a bug in this module, not
/// something to paper over.
pub fn read_jsonl(path: &Path) -> anyhow::Result<Vec<Envelope>> {
    let contents = std::fs::read_to_string(path)?;
    let mut events = VecDeque::new();
    for line in contents.lines() {
        if line.trim().is_empty() {
            continue;
        }
        events.push_back(serde_json::from_str::<Envelope>(line)?);
    }
    Ok(events.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::thread;

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::telemetry::event::{AgentState, RunStart, ToolCall};

    fn fixture_tool_call(argument_text: &str, result_text: &str) -> TelemetryEvent {
        TelemetryEvent::ToolCall(ToolCall {
            call_id: "call-1".to_string(),
            name: "shell".to_string(),
            arguments: Truncated::whole(argument_text),
            success: true,
            duration_ms: 12,
            result_size_chars: result_text.chars().count(),
            result_summary: Truncated::whole(result_text),
            handle: None,
        })
    }

    #[test]
    fn test_noop_sink_drops_nothing_and_writes_nothing() {
        let fixture = Sink::noop();

        fixture.emit(
            TelemetryEvent::RunStart(RunStart { task_id: None, repo_root: "/repo".to_string() }),
            None,
            None,
        );

        assert_eq!(fixture.dropped_events(), 0);
    }

    #[test]
    fn test_jsonl_sink_round_trips_events() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("telemetry.jsonl");
        let sink = Sink::jsonl(path.clone(), "run-1", SinkLimits::default());

        sink.emit(
            TelemetryEvent::RunStart(RunStart {
                task_id: Some("issue-1".to_string()),
                repo_root: "/repo".to_string(),
            }),
            Some("conv-1".to_string()),
            None,
        );
        sink.emit(
            TelemetryEvent::AgentState(AgentState {
                from: None,
                to: "executing".to_string(),
                reason: None,
                iteration: Some(1),
            }),
            Some("conv-1".to_string()),
            None,
        );

        let actual = read_jsonl(&path).unwrap();

        assert_eq!(actual.len(), 2);
        assert_eq!(actual[0].seq, 0);
        assert_eq!(actual[1].seq, 1);
        assert_eq!(actual[0].run_id, "run-1");
        assert_eq!(actual[0].conversation_id, Some("conv-1".to_string()));
        assert_eq!(
            actual[0].event,
            TelemetryEvent::RunStart(RunStart {
                task_id: Some("issue-1".to_string()),
                repo_root: "/repo".to_string(),
            })
        );
        assert_eq!(
            actual[1].event,
            TelemetryEvent::AgentState(AgentState {
                from: None,
                to: "executing".to_string(),
                reason: None,
                iteration: Some(1),
            })
        );
    }

    #[test]
    fn test_write_failure_is_swallowed_and_counted_not_propagated() {
        // A path inside a directory that does not exist: every write fails,
        // but emit() must never panic or return an error.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("does-not-exist").join("telemetry.jsonl");
        let sink = Sink::jsonl(path, "run-1", SinkLimits::default());

        sink.emit(
            TelemetryEvent::RunStart(RunStart { task_id: None, repo_root: "/repo".to_string() }),
            None,
            None,
        );
        sink.emit(
            TelemetryEvent::RunStart(RunStart { task_id: None, repo_root: "/repo".to_string() }),
            None,
            None,
        );

        assert_eq!(sink.dropped_events(), 2);
    }

    #[test]
    fn test_sequence_numbers_strictly_increase_under_concurrent_emits() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("telemetry.jsonl");
        let sink = Arc::new(Sink::jsonl(path.clone(), "run-1", SinkLimits::default()));

        let handles: Vec<_> = (0..8)
            .map(|_| {
                let sink = Arc::clone(&sink);
                thread::spawn(move || {
                    for _ in 0..25 {
                        sink.emit(
                            TelemetryEvent::RunStart(RunStart {
                                task_id: None,
                                repo_root: "/repo".to_string(),
                            }),
                            None,
                            None,
                        );
                    }
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap();
        }

        let actual = read_jsonl(&path).unwrap();
        let mut sequences: Vec<u64> = actual.iter().map(|envelope| envelope.seq).collect();
        sequences.sort_unstable();

        let expected: Vec<u64> = (0..200).collect();
        assert_eq!(sequences, expected);
        assert_eq!(sink.dropped_events(), 0);
    }

    #[test]
    fn test_sink_truncates_large_arguments_and_marks_it_visibly() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("telemetry.jsonl");
        let limits = SinkLimits { max_argument_chars: 10, max_result_chars: 10 };
        let sink = Sink::jsonl(path.clone(), "run-1", limits);
        let long_argument = "x".repeat(100);
        let long_result = "y".repeat(200);

        sink.emit(fixture_tool_call(&long_argument, &long_result), None, None);

        let actual = read_jsonl(&path).unwrap();
        let TelemetryEvent::ToolCall(tool_call) = &actual[0].event else {
            panic!("expected a tool_call event");
        };
        assert!(tool_call.arguments.truncated);
        assert_eq!(tool_call.arguments.text.chars().count(), 10);
        assert_eq!(tool_call.arguments.original_chars, 100);
        assert!(tool_call.result_summary.truncated);
        assert_eq!(tool_call.result_summary.text.chars().count(), 10);
        assert_eq!(tool_call.result_summary.original_chars, 200);
        // The full size is on record independent of the truncated summary.
        assert_eq!(tool_call.result_size_chars, 200);
    }

    #[test]
    fn test_sink_leaves_small_arguments_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("telemetry.jsonl");
        let sink = Sink::jsonl(path.clone(), "run-1", SinkLimits::default());

        sink.emit(fixture_tool_call("short args", "short result"), None, None);

        let actual = read_jsonl(&path).unwrap();
        let TelemetryEvent::ToolCall(tool_call) = &actual[0].event else {
            panic!("expected a tool_call event");
        };
        assert!(!tool_call.arguments.truncated);
        assert_eq!(tool_call.arguments.text, "short args");
        assert!(!tool_call.result_summary.truncated);
        assert_eq!(tool_call.result_summary.text, "short result");
    }
}
