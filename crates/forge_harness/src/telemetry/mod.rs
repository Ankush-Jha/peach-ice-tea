//! The telemetry event stream: an append-only record of what a run actually
//! did (`ALIGNMENT.md` R-HACK-3, `HACKATHON.md` §13.3, §15).
//!
//! `event.rs` defines the contract (the envelope and the event enum),
//! `sink.rs` writes it out, and `organizer_adapter.rs` will translate it to
//! the organizers' schema once published (`DECISIONS.md` D-020). This module
//! is the seam between them and every future caller: install a [`Sink`] once
//! per process, then call [`emit`] from wherever something worth recording
//! happens.
//!
//! Nothing is installed by default, mirroring [`crate::runtime`]: until a
//! caller installs a sink, [`emit`] does nothing, so interactive use and
//! every test that never opts in behaves exactly as it did before this
//! module existed. Wiring an installed sink into the orchestrator's hooks is
//! a separate, later piece of work — this module only provides the API.

pub mod event;
pub mod organizer_adapter;
pub mod sink;

use std::sync::OnceLock;

pub use event::{Envelope, TelemetryEvent};
pub use sink::{Sink, SinkLimits};

static SINK: OnceLock<Sink> = OnceLock::new();

/// Installs the telemetry sink for this process.
///
/// Returns `false` if one was already installed, which is not treated as an
/// error: the first installation wins, the same convention as
/// [`crate::runtime::install`]. That said, calling `install` twice is
/// almost certainly a caller bug (two sessions racing to install in one
/// process), so — unlike `crate::runtime::install`, which stays silent —
/// this one logs it: every other failure path in this module already warns
/// on something going wrong, and a silently-ignored second sink was the one
/// path that didn't.
pub fn install(sink: Sink) -> bool {
    install_into(&SINK, sink)
}

/// The logic behind [`install`], parameterised over the cell it installs
/// into.
///
/// Exists so tests can exercise "install, then install again" without
/// touching the process-global [`SINK`]: two tests racing to write to the
/// same real global from different threads (the default `cargo test`
/// execution model — nextest isolates each test into its own process, but
/// that isolation is not guaranteed here) would make either test's outcome
/// depend on who won the race, which is exactly the kind of flake this
/// module cannot afford to ship.
fn install_into(cell: &OnceLock<Sink>, sink: Sink) -> bool {
    let installed = cell.set(sink).is_ok();
    if !installed {
        tracing::warn!(
            "telemetry: install() called again after a sink was already installed for this \
             process; ignoring this one and keeping the first"
        );
    }
    installed
}

/// Whether a sink has been installed for this process.
pub fn is_installed() -> bool {
    SINK.get().is_some()
}

/// Records one event with no conversation or agent tag.
///
/// A no-op when no sink has been installed.
pub fn emit(event: TelemetryEvent) {
    emit_with(event, None, None)
}

/// Records one event, tagging it with `conversation_id`/`agent_id` when the
/// run has more than one conversation or agent (subagents, resumed
/// sessions).
///
/// A no-op when no sink has been installed.
pub fn emit_with(event: TelemetryEvent, conversation_id: Option<String>, agent_id: Option<String>) {
    let Some(sink) = SINK.get() else {
        return;
    };
    sink.emit(event, conversation_id, agent_id);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::telemetry::event::RunStart;

    #[test]
    fn test_emit_is_a_no_op_when_no_sink_is_installed() {
        // This process installs no sink in this test module (installing one
        // would permanently affect every other test sharing this binary,
        // the same constraint `crate::runtime`'s own tests observe). `emit`
        // must not panic and must have nothing to assert on either side.
        assert!(!is_installed());

        emit(TelemetryEvent::RunStart(RunStart { task_id: None, repo_root: "/repo".to_string() }));
        emit_with(
            TelemetryEvent::RunStart(RunStart { task_id: None, repo_root: "/repo".to_string() }),
            Some("conv-1".to_string()),
            None,
        );
    }

    #[test]
    fn test_a_second_install_is_refused() {
        // Uses a private `OnceLock`, not the process-global `SINK`: this
        // process shares a single test binary and, under plain `cargo
        // test`, a single process across every test, so installing into
        // the real global here would race with
        // `test_emit_is_a_no_op_when_no_sink_is_installed` above (which
        // depends on nothing else in the process having installed a sink
        // yet) and make either test's outcome depend on thread scheduling.
        // `install_into` carries the exact logic `install` calls with
        // `&SINK`, so this still exercises the real behaviour.
        let cell = OnceLock::new();

        let first = install_into(&cell, Sink::noop());
        let second = install_into(&cell, Sink::noop());

        assert!(first);
        assert!(!second);
    }
}
