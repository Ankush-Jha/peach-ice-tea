//! harness: the start and finish of the harness around one `peach exec` run.
//!
//! Kept out of `ui.rs` so the upstream file carries two call sites rather than
//! the whole lifecycle (`DECISIONS.md` D-004). Everything here fails open
//! (`CLAUDE.md` principle 5): a snapshot or telemetry failure degrades what
//! the evidence can show, never whether the task runs.

use std::path::{Path, PathBuf};

use peach_domain::{ExecIntegrity, ExecIntegrityViolation};
use peach_harness::integrity::{self, IntegrityReport, Manifest, ViolationKind};
use peach_harness::telemetry::{self, TelemetryEvent, event};
use peach_harness::runtime;

/// Harness state held for the length of one `exec` run.
pub struct ExecHarness {
    repo_root: PathBuf,
    protect_globs: Vec<String>,
    exclude_globs: Vec<String>,
    manifest: Manifest,
    /// Copies of the protected files, taken before the run. Held here so the
    /// directory lives exactly as long as the run and is removed afterwards;
    /// without it a violation can be detected but never undone.
    _snapshot: Option<tempfile::TempDir>,
}

impl ExecHarness {
    /// Captures the integrity manifest, installs the runtime (activating the
    /// dispatch-time guard) and, when `telemetry_path` is given, the JSONL
    /// telemetry sink. Returns the harness and the protected-file notice for
    /// the model, when there is anything to protect.
    ///
    /// # Arguments
    /// * `repo_root` - Repository the task operates on.
    /// * `telemetry_path` - File to append telemetry events to, if any.
    pub fn start(repo_root: PathBuf, telemetry_path: Option<&Path>) -> (Self, Option<String>) {
        if let Some(path) = telemetry_path {
            telemetry::install(telemetry::Sink::jsonl(
                path,
                run_id(),
                telemetry::SinkLimits::default(),
            ));
        }

        let protect_globs: Vec<String> =
            integrity::DEFAULT_PROTECTED_GLOBS.iter().map(|s| s.to_string()).collect();
        let exclude_globs: Vec<String> =
            integrity::DEFAULT_EXCLUDE_GLOBS.iter().map(|s| s.to_string()).collect();

        // Outside the repository by construction (the system temp dir), so the
        // snapshot can never itself show up as a change to the repo or be
        // reached by the agent's own relative-path edits.
        let snapshot = tempfile::Builder::new()
            .prefix("peach-ice-tea-integrity-")
            .tempdir()
            .inspect_err(|error| {
                tracing::warn!(?error, "No integrity snapshot dir; violations will be reported but not restored")
            })
            .ok();
        let (protected, manifest) = integrity::discover_and_capture(
            &repo_root,
            &protect_globs,
            &exclude_globs,
            snapshot.as_ref().map(tempfile::TempDir::path),
        );
        let notice = integrity::model_notice(&protected);

        runtime::install(
            runtime::HarnessRuntime::new(repo_root.clone())
                .non_interactive(true)
                .protected(protected),
        );

        telemetry::emit(TelemetryEvent::RunStart(event::RunStart {
            task_id: None,
            repo_root: repo_root.display().to_string(),
        }));
        // RunStart marks the point from which the guard is live. Flushed so an
        // outside observer (the eval runner) can rely on that ordering.
        telemetry::flush();

        (
            Self { repo_root, protect_globs, exclude_globs, manifest, _snapshot: snapshot },
            notice,
        )
    }

    /// Verifies protected files against the pre-run manifest, restores any
    /// that changed, and records the result and the end of the run in
    /// telemetry. Must run after the agent has stopped, on every exit path.
    ///
    /// # Arguments
    /// * `outcome` - How the run ended, as reported in the `exec` JSON.
    /// * `duration_ms` - Wall-clock duration of the run.
    pub fn finish(self, outcome: &str, duration_ms: u64) -> ExecIntegrity {
        let report = integrity::verify_and_restore(
            &self.repo_root,
            &self.protect_globs,
            &self.exclude_globs,
            &self.manifest,
        );
        for event in integrity::telemetry_events(&report) {
            telemetry::emit(event);
        }
        telemetry::emit(TelemetryEvent::RunEnd(event::RunEnd {
            outcome: outcome.to_string(),
            duration_ms,
            dropped_events: telemetry::dropped_events(),
        }));
        telemetry::flush();
        to_exec_integrity(&report)
    }
}

/// Identifier for this run's telemetry: start time plus process id, unique
/// per machine without needing a conversation id, which does not exist yet
/// when the run starts.
fn run_id() -> String {
    format!("{}-{}", chrono::Utc::now().format("%Y%m%dT%H%M%S%.3fZ"), std::process::id())
}

fn to_exec_integrity(report: &IntegrityReport) -> ExecIntegrity {
    ExecIntegrity {
        checked: report.checked,
        violations: report
            .violations
            .iter()
            .map(|violation| ExecIntegrityViolation {
                path: violation.path.clone(),
                kind: match violation.kind {
                    ViolationKind::Modified => "modified",
                    ViolationKind::Deleted => "deleted",
                    ViolationKind::Added => "added",
                }
                .to_string(),
                restored: violation.restored,
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use peach_harness::integrity::Violation;
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_integrity_report_maps_onto_the_exec_report_shape() {
        let fixture = IntegrityReport {
            violations: vec![Violation {
                path: "tests/test_math.py".to_string(),
                kind: ViolationKind::Added,
                restored: true,
            }],
            checked: 4,
        };

        let actual = to_exec_integrity(&fixture);

        let expected = ExecIntegrity {
            checked: 4,
            violations: vec![ExecIntegrityViolation {
                path: "tests/test_math.py".to_string(),
                kind: "added".to_string(),
                restored: true,
            }],
        };
        assert_eq!(actual, expected);
    }
}
