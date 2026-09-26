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
use std::time::{Duration, Instant};

use crate::identity::{HarnessIdentity, TELEMETRY_SCHEMA_VERSION};
use crate::telemetry::event::{Envelope, TelemetryEvent, Truncated};

/// Identifies the internal telemetry schema in every envelope
/// (`organizer_adapter.rs` maps to the organizers' schema separately;
/// `DECISIONS.md` D-020).
pub const SCHEMA_ID: &str = "peach-ice-tea.telemetry";

/// Configurable ceilings and flush policy for one sink.
///
/// The text limits are applied on write, on top of whatever the caller
/// already put in a [`Truncated`] field, so one enormous tool result cannot
/// make the log unusable regardless of what produced it. The flush fields
/// trade a small, bounded amount of crash-loss risk for far fewer write
/// syscalls in the agent's hot loop: flushing on every event (the original
/// design) cost one syscall per event (~180µs measured with 3KB payloads),
/// which is wall-clock that §19 scores. Buffering means a hard crash
/// (`SIGKILL`, power loss) between flushes can lose up to
/// `flush_every_events` events or `flush_interval` of wall-clock time's
/// worth of events, whichever threshold is reached first. A `RunEnd` event
/// always forces an immediate flush, and the sink also flushes on `Drop`, so
/// a normal process exit — including a panic that unwinds rather than
/// aborts — never loses a buffered event; only an abrupt crash can.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SinkLimits {
    /// Maximum characters kept for a tool call's `arguments`.
    pub max_argument_chars: usize,
    /// Maximum characters kept for a tool call's `result_summary`.
    pub max_result_chars: usize,
    /// Flush after this many buffered events, even if `flush_interval`
    /// hasn't elapsed yet.
    pub flush_every_events: usize,
    /// Flush after this much wall-clock time since the last flush, even if
    /// `flush_every_events` hasn't been reached yet.
    pub flush_interval: Duration,
}

impl Default for SinkLimits {
    fn default() -> Self {
        Self {
            max_argument_chars: 2_000,
            max_result_chars: 4_000,
            flush_every_events: 20,
            flush_interval: Duration::from_millis(500),
        }
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

    /// Forces any buffered events to disk. A no-op for [`Sink::NoOp`].
    ///
    /// Normally unnecessary: a `RunEnd` event always flushes, and so does
    /// dropping the sink. Exists for callers (tests, and anything that
    /// reads the log while the run is still in progress) that need the file
    /// to reflect everything emitted so far right now.
    pub fn flush(&self) {
        if let Sink::Jsonl(sink) = self {
            sink.flush();
        }
    }
}

/// Sequencing and buffering state for the JSONL sink, held under one lock.
///
/// The sequence number and the write happen inside the same critical
/// section (see [`JsonlSink::write_line`]) so that file order always agrees
/// with `seq` order — allocating the sequence number before taking the lock
/// let two threads race, so whichever won the lock could write a lower `seq`
/// after a higher one was already on disk.
struct JsonlState {
    writer: Option<BufWriter<std::fs::File>>,
    next_seq: u64,
    unflushed_events: usize,
    last_flush: Instant,
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
    dropped: AtomicU64,
    state: Mutex<JsonlState>,
}

impl JsonlSink {
    fn new(path: PathBuf, run_id: String, limits: SinkLimits) -> Self {
        Self {
            path,
            run_id,
            limits,
            dropped: AtomicU64::new(0),
            state: Mutex::new(JsonlState {
                writer: None,
                next_seq: 0,
                unflushed_events: 0,
                last_flush: Instant::now(),
            }),
        }
    }

    fn emit(&self, event: TelemetryEvent, conversation_id: Option<String>, agent_id: Option<String>) {
        // A RunEnd is the last event of a run: force it to disk immediately
        // rather than leaving it in the buffer for a flush that may never
        // come (SHOULD-FIX 6).
        let force_flush = matches!(event, TelemetryEvent::RunEnd(_));
        let event = enforce_limits(event, self.limits);

        if let Err(error) = self.write_line(event, conversation_id, agent_id, force_flush) {
            self.dropped.fetch_add(1, Ordering::Relaxed);
            tracing::warn!(
                error = %error,
                path = %self.path.display(),
                "telemetry: dropping event, sink write failed (fail-open)"
            );
        }
    }

    /// Builds the envelope, appends it to the buffer, and flushes if this
    /// write pushed the sink past its size or time threshold.
    ///
    /// Sequence allocation, the append and the flush decision all happen
    /// while holding `state`'s lock, in that order, so no other thread can
    /// interleave a write between this event's `seq` being chosen and it
    /// landing in the file (BLOCKING 2: file order must agree with `seq`
    /// order).
    fn write_line(
        &self,
        event: TelemetryEvent,
        conversation_id: Option<String>,
        agent_id: Option<String>,
        force_flush: bool,
    ) -> anyhow::Result<()> {
        let mut guard =
            self.state.lock().map_err(|_| anyhow::anyhow!("telemetry sink lock poisoned"))?;

        let seq = guard.next_seq;
        // Reserved unconditionally, even if the write below fails: ordering
        // must stay monotonic for the events that end up dropped too, so a
        // consumer can tell from a gap that something was lost rather than
        // being silently misled about ordering.
        guard.next_seq += 1;

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
        let line = serde_json::to_string(&envelope)?;

        if guard.writer.is_none() {
            let file = OpenOptions::new().create(true).append(true).open(&self.path)?;
            guard.writer = Some(BufWriter::new(file));
        }

        guard.unflushed_events += 1;
        let due_by_count = guard.unflushed_events >= self.limits.flush_every_events;
        let due_by_time = guard.last_flush.elapsed() >= self.limits.flush_interval;
        let should_flush = force_flush || due_by_count || due_by_time;

        // `is_some()` was just established above; this cannot panic.
        let writer = guard.writer.as_mut().expect("sink writer initialised above");
        writer.write_all(line.as_bytes())?;
        writer.write_all(b"\n")?;
        if should_flush {
            writer.flush()?;
            guard.unflushed_events = 0;
            guard.last_flush = Instant::now();
        }
        Ok(())
    }

    fn flush(&self) {
        let Ok(mut guard) = self.state.lock() else {
            return;
        };
        if let Some(writer) = guard.writer.as_mut() {
            match writer.flush() {
                Ok(()) => {
                    guard.unflushed_events = 0;
                    guard.last_flush = Instant::now();
                }
                Err(error) => {
                    tracing::warn!(
                        error = %error,
                        path = %self.path.display(),
                        "telemetry: explicit flush failed (fail-open)"
                    );
                }
            }
        }
    }
}

impl Drop for JsonlSink {
    /// Flushes any buffered events, so a normal process exit — including an
    /// unwinding panic — never loses them to the buffering introduced for
    /// SHOULD-FIX 6. Only an abrupt crash (`SIGKILL`, power loss) can still
    /// lose the events buffered since the last flush.
    fn drop(&mut self) {
        self.flush();
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
/// Strict: the first line that fails to parse is returned as an error.
/// Meant for tests, where a parse failure is a bug in this module to catch,
/// not something to route around. The report generator (R-HACK-4/TH.5)
/// must NOT use this function — see [`read_jsonl_lenient`].
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

/// Reads a JSONL telemetry file back into its events, in file order,
/// tolerating lines that fail to parse.
///
/// A disk-full condition, a process killed mid-write, or any other crash
/// between the sink's buffered writer and the OS can leave a truncated line
/// at the end of the file, or (more rarely) a corrupted line in the middle.
/// The report this feeds (R-HACK-4/TH.5) is the evidence a judge scores the
/// run on, so one bad line must cost the report one entry, not the whole
/// bundle: fail-open applies to *reading* the log, not only to writing it
/// (`CLAUDE.md` principle 5). Returns every line that did parse, in file
/// order, plus how many did not — so a report can say "3 events unreadable"
/// instead of rendering nothing.
pub fn read_jsonl_lenient(path: &Path) -> anyhow::Result<(Vec<Envelope>, usize)> {
    let contents = std::fs::read_to_string(path)?;
    let mut events = Vec::new();
    let mut skipped = 0usize;
    for line in contents.lines() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<Envelope>(line) {
            Ok(envelope) => events.push(envelope),
            Err(error) => {
                skipped += 1;
                tracing::warn!(
                    error = %error,
                    path = %path.display(),
                    "telemetry: skipping unparseable line while reading log"
                );
            }
        }
    }
    Ok((events, skipped))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::thread;

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::telemetry::event::{AgentState, RunEnd, RunStart, ToolCall};

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
            origin_call_id: None,
        })
    }

    /// Flush limits that never trigger on their own, so a test controls
    /// exactly when data reaches disk via an explicit `sink.flush()` (or a
    /// `RunEnd`) rather than incidentally crossing a threshold.
    fn never_auto_flush() -> SinkLimits {
        SinkLimits {
            flush_every_events: usize::MAX,
            flush_interval: Duration::from_secs(3600),
            ..SinkLimits::default()
        }
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
        let sink = Sink::jsonl(path.clone(), "run-1", never_auto_flush());

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
        sink.flush();

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

    /// Emits `threads * per_thread` `RunStart` events concurrently from
    /// `threads` threads, sharing one sink, and returns the sink alongside
    /// the temp directory it wrote into (kept alive: dropping it would
    /// delete the file out from under the caller) and the file's path.
    fn emit_concurrently(
        threads: usize,
        per_thread: usize,
        limits: SinkLimits,
    ) -> (Arc<Sink>, tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("telemetry.jsonl");
        let sink = Arc::new(Sink::jsonl(path.clone(), "run-1", limits));

        let handles: Vec<_> = (0..threads)
            .map(|_| {
                let sink = Arc::clone(&sink);
                thread::spawn(move || {
                    for _ in 0..per_thread {
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
        (sink, dir, path)
    }

    #[test]
    fn test_sequence_numbers_strictly_increase_under_concurrent_emits() {
        let (sink, _dir, path) = emit_concurrently(8, 25, SinkLimits::default());
        sink.flush();

        let actual = read_jsonl(&path).unwrap();
        let mut sequences: Vec<u64> = actual.iter().map(|envelope| envelope.seq).collect();
        sequences.sort_unstable();

        let expected: Vec<u64> = (0..200).collect();
        assert_eq!(sequences, expected);
        assert_eq!(sink.dropped_events(), 0);
    }

    #[test]
    fn test_file_order_matches_seq_order_under_concurrent_emits() {
        // BLOCKING 2 regression test: the earlier implementation allocated
        // `seq` with a plain `fetch_add` *before* taking the writer lock, so
        // a thread holding a higher `seq` could win the lock and land in
        // the file first. Reproduced upstream with 32 threads x 200 emits;
        // 32x50 here is enough to make the race near-certain without this
        // test taking noticeably longer to run. Deliberately does NOT sort
        // before comparing: sorting is exactly what would hide this bug.
        let (sink, _dir, path) = emit_concurrently(32, 50, SinkLimits::default());
        sink.flush();

        let actual = read_jsonl(&path).unwrap();
        let sequences_in_file_order: Vec<u64> = actual.iter().map(|envelope| envelope.seq).collect();

        let expected_in_file_order: Vec<u64> = (0..1_600).collect();
        assert_eq!(sequences_in_file_order, expected_in_file_order);
    }

    #[test]
    fn test_sink_truncates_large_arguments_and_marks_it_visibly() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("telemetry.jsonl");
        let limits = SinkLimits { max_argument_chars: 10, max_result_chars: 10, ..never_auto_flush() };
        let sink = Sink::jsonl(path.clone(), "run-1", limits);
        let long_argument = "x".repeat(100);
        let long_result = "y".repeat(200);

        sink.emit(fixture_tool_call(&long_argument, &long_result), None, None);
        sink.flush();

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
    fn test_read_jsonl_strict_fails_on_a_corrupted_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("telemetry.jsonl");
        let sink = Sink::jsonl(path.clone(), "run-1", never_auto_flush());
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
        sink.flush();
        // Simulate a crash mid-write: a truncated, unparseable final line
        // appended straight to the file, bypassing the sink.
        let mut raw = OpenOptions::new().append(true).open(&path).unwrap();
        raw.write_all(b"{\"schema_id\": \"peach-ice-tea.telemetry\", \"trunc").unwrap();

        let actual = read_jsonl(&path);

        assert!(actual.is_err());
    }

    #[test]
    fn test_read_jsonl_lenient_skips_and_counts_a_corrupted_line_instead_of_losing_everything() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("telemetry.jsonl");
        let sink = Sink::jsonl(path.clone(), "run-1", never_auto_flush());
        sink.emit(
            TelemetryEvent::RunStart(RunStart {
                task_id: Some("issue-1".to_string()),
                repo_root: "/repo".to_string(),
            }),
            None,
            None,
        );
        sink.emit(
            TelemetryEvent::RunStart(RunStart {
                task_id: Some("issue-2".to_string()),
                repo_root: "/repo".to_string(),
            }),
            None,
            None,
        );
        sink.flush();
        let mut raw = OpenOptions::new().append(true).open(&path).unwrap();
        raw.write_all(b"{\"schema_id\": \"peach-ice-tea.telemetry\", \"trunc").unwrap();

        let (actual_events, actual_skipped) = read_jsonl_lenient(&path).unwrap();

        assert_eq!(actual_events.len(), 2);
        assert_eq!(actual_skipped, 1);
        assert_eq!(
            actual_events[0].event,
            TelemetryEvent::RunStart(RunStart {
                task_id: Some("issue-1".to_string()),
                repo_root: "/repo".to_string(),
            })
        );
        assert_eq!(
            actual_events[1].event,
            TelemetryEvent::RunStart(RunStart {
                task_id: Some("issue-2".to_string()),
                repo_root: "/repo".to_string(),
            })
        );
    }

    #[test]
    fn test_sink_leaves_small_arguments_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("telemetry.jsonl");
        let sink = Sink::jsonl(path.clone(), "run-1", never_auto_flush());

        sink.emit(fixture_tool_call("short args", "short result"), None, None);
        sink.flush();

        let actual = read_jsonl(&path).unwrap();
        let TelemetryEvent::ToolCall(tool_call) = &actual[0].event else {
            panic!("expected a tool_call event");
        };
        assert!(!tool_call.arguments.truncated);
        assert_eq!(tool_call.arguments.text, "short args");
        assert!(!tool_call.result_summary.truncated);
        assert_eq!(tool_call.result_summary.text, "short result");
    }

    #[test]
    fn test_flush_is_deferred_until_a_threshold_or_run_end_forces_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("telemetry.jsonl");
        let sink = Sink::jsonl(path.clone(), "run-1", never_auto_flush());

        sink.emit(
            TelemetryEvent::RunStart(RunStart { task_id: None, repo_root: "/repo".to_string() }),
            None,
            None,
        );
        sink.emit(
            TelemetryEvent::AgentState(AgentState {
                from: None,
                to: "executing".to_string(),
                reason: None,
                iteration: None,
            }),
            None,
            None,
        );
        // Neither threshold has been reached (both are effectively
        // disabled), so nothing has reached disk yet: this is the buffering
        // SHOULD-FIX 6 introduces, made observable.
        let buffered_so_far = std::fs::read_to_string(&path).unwrap_or_default();
        assert_eq!(buffered_so_far, "");

        // RunEnd always forces a flush, regardless of the thresholds.
        sink.emit(
            TelemetryEvent::RunEnd(RunEnd {
                outcome: "completed".to_string(),
                duration_ms: 10,
                dropped_events: 0,
            }),
            None,
            None,
        );

        let actual = read_jsonl(&path).unwrap();
        assert_eq!(actual.len(), 3);
    }

    #[test]
    fn test_flush_triggers_automatically_at_the_event_count_threshold() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("telemetry.jsonl");
        let limits = SinkLimits { flush_every_events: 3, ..never_auto_flush() };
        let sink = Sink::jsonl(path.clone(), "run-1", limits);

        for _ in 0..3 {
            sink.emit(
                TelemetryEvent::RunStart(RunStart { task_id: None, repo_root: "/repo".to_string() }),
                None,
                None,
            );
        }

        // No explicit flush() and no RunEnd: the count threshold alone must
        // have pushed these to disk.
        let actual = read_jsonl(&path).unwrap();
        assert_eq!(actual.len(), 3);
    }

    #[test]
    fn test_flush_triggers_automatically_after_the_time_threshold() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("telemetry.jsonl");
        let limits = SinkLimits {
            flush_every_events: usize::MAX,
            flush_interval: Duration::from_millis(10),
            ..SinkLimits::default()
        };
        let sink = Sink::jsonl(path.clone(), "run-1", limits);

        sink.emit(
            TelemetryEvent::RunStart(RunStart { task_id: None, repo_root: "/repo".to_string() }),
            None,
            None,
        );
        thread::sleep(Duration::from_millis(25));
        // This second emit is the one that observes the elapsed time and
        // decides to flush; it carries the first event to disk with it.
        sink.emit(
            TelemetryEvent::RunStart(RunStart { task_id: None, repo_root: "/repo".to_string() }),
            None,
            None,
        );

        let actual = read_jsonl(&path).unwrap();
        assert_eq!(actual.len(), 2);
    }
}
