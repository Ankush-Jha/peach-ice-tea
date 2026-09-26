//! harness: the start and finish of the harness around one `peach exec` run.
//!
//! Kept out of `ui.rs` so the upstream file carries two call sites rather than
//! the whole lifecycle (`DECISIONS.md` D-004). Everything here fails open
//! (`CLAUDE.md` principle 5): a snapshot or telemetry failure degrades what
//! the evidence can show, never whether the task runs.

use std::path::{Path, PathBuf};

use peach_domain::{ExecIntegrity, ExecIntegrityViolation, ExecReport};
use peach_harness::evidence::{self, Evidence};
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
    /// The evidence bundle, when `--evidence-dir` was given (R-HACK-5).
    evidence: Option<Evidence>,
    /// Commit the repository was at when the run started, for the diff.
    start_commit: Option<String>,
    /// The repository's test command, explicit or detected.
    test_command: Option<peach_harness::verify::TestCommand>,
    started_at: String,
}

/// Where `exec` writes its evidence, from its command-line flags.
#[derive(Debug, Default)]
pub struct ExecOutputs<'a> {
    /// `--evidence-dir`: the bundle directory.
    pub evidence_dir: Option<&'a Path>,
    /// `--telemetry`: overrides `<evidence-dir>/telemetry.jsonl`.
    pub telemetry: Option<&'a Path>,
    /// `--test-command`: overrides detection (R-HACK-7).
    pub test_command: Option<&'a str>,
}

/// Wall-clock limit for the harness's final test run, in seconds, unless
/// `PEACH_HARNESS_FINAL_TEST_TIMEOUT_SECS` says otherwise.
const FINAL_TEST_TIMEOUT_SECS: u64 = 300;

impl ExecHarness {
    /// Captures the integrity manifest, installs the runtime (activating the
    /// dispatch-time guard), opens the evidence bundle and the telemetry sink
    /// when asked for, and records the frozen prompt. Returns the harness and
    /// the protected-file notice for the model, when there is anything to
    /// protect.
    ///
    /// # Arguments
    /// * `repo_root` - Repository the task operates on.
    /// * `prompt` - The task exactly as given, before any harness notice.
    /// * `outputs` - Where evidence and telemetry go.
    pub fn start(repo_root: PathBuf, prompt: &str, outputs: ExecOutputs<'_>) -> (Self, Option<String>) {
        let started_at = evidence::now();
        let mut evidence = outputs.evidence_dir.and_then(Evidence::create);
        if let Some(evidence) = evidence.as_mut() {
            evidence.write_text(evidence::PROMPT, prompt);
        }
        let telemetry_path = outputs
            .telemetry
            .map(Path::to_path_buf)
            .or_else(|| evidence.as_ref().map(|evidence| evidence.path(evidence::TELEMETRY)));
        let start_commit = evidence::git_head(&repo_root);
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
        let notice = match (integrity::model_notice(&protected), integrity::test_config_notice(&manifest)) {
            (Some(files), Some(config)) => Some(format!("{files}\n\n{config}")),
            (files, config) => files.or(config),
        };

        // TH.5 (D-056): written now, because SIGKILL leaves no chance later.
        if let Some(evidence) = evidence.as_mut() {
            evidence.write_json(evidence::INTEGRITY_BASELINE, &manifest);
            evidence.write_provisional_manifest(&started_at);
        }

        let test_command = peach_harness::verify::detect(&repo_root, outputs.test_command);
        runtime::install(
            runtime::HarnessRuntime::new(repo_root.clone())
                .non_interactive(true)
                .protected(protected)
                .test_command_is(test_command.clone()),
        );

        telemetry::emit(TelemetryEvent::RunStart(event::RunStart {
            task_id: None,
            repo_root: repo_root.display().to_string(),
        }));
        // RunStart marks the point from which the guard is live. Flushed so an
        // outside observer (the eval runner) can rely on that ordering.
        telemetry::flush();

        (
            Self {
                repo_root,
                protect_globs,
                exclude_globs,
                manifest,
                _snapshot: snapshot,
                evidence,
                start_commit,
                test_command,
                started_at,
            },
            notice,
        )
    }

    /// Verifies protected files against the pre-run manifest, restores any
    /// that changed, records the result in telemetry, and writes the
    /// integrity, diff and test files of the bundle — in that order, so the
    /// diff reflects the restored tree. Must run after the agent has stopped,
    /// on every exit path; [`Self::seal`] completes the bundle.
    pub fn finish(&mut self, interrupted: bool) -> ExecIntegrity {
        let report = integrity::verify_and_restore(
            &self.repo_root,
            &self.protect_globs,
            &self.exclude_globs,
            &self.manifest,
        );
        for event in integrity::telemetry_events(&report) {
            telemetry::emit(event);
        }
        if let Some(evidence) = self.evidence.as_mut() {
            evidence.write_json(evidence::INTEGRITY, &report);
            evidence.write_diff(&self.repo_root, self.start_commit.as_deref());
            match &self.test_command {
                // Whoever stopped the run wants it to end now; a full test
                // run could take minutes. Integrity and the diff are cheap.
                Some(_) if interrupted => evidence.write_json(
                    evidence::TESTS,
                    &serde_json::json!({
                        "ran": false,
                        "reason": "the run was interrupted by a signal; the final test run was skipped",
                    }),
                ),
                Some(test) => {
                    let timeout = std::env::var("PEACH_HARNESS_FINAL_TEST_TIMEOUT_SECS")
                        .ok()
                        .and_then(|secs| secs.parse().ok())
                        .unwrap_or(FINAL_TEST_TIMEOUT_SECS);
                    let run = peach_harness::verify::run_final(
                        &self.repo_root,
                        test,
                        std::time::Duration::from_secs(timeout),
                    );
                    evidence.write_json(evidence::TESTS, &run);
                    // A suite can write files of its own (snapshots, fixtures);
                    // the submitted tree must still hold the original tests.
                    let after = integrity::verify_and_restore(
                        &self.repo_root,
                        &self.protect_globs,
                        &self.exclude_globs,
                        &self.manifest,
                    );
                    if !after.is_clean() {
                        for event in integrity::telemetry_events(&after) {
                            telemetry::emit(event);
                        }
                        evidence.note(format!(
                            "the final test run changed {} protected file(s); restored: {}",
                            after.violations.len(),
                            after.violations.iter().map(|v| v.path.as_str()).collect::<Vec<_>>().join(", ")
                        ));
                    }
                }
                None => evidence.write_json(
                    evidence::TESTS,
                    &serde_json::json!({
                        "ran": false,
                        "reason": "no test command was given (--test-command) or detected in the repository",
                    }),
                ),
            }
        }
        to_exec_integrity(&report)
    }

    /// Writes the transcript and the exec outcome, ends the telemetry stream,
    /// and writes the manifest last so it covers every other file.
    ///
    /// # Arguments
    /// * `transcript` - The conversation and its subagent conversations.
    /// * `report` - The `exec` outcome line.
    /// * `outcome` - How the run ended.
    /// * `duration_ms` - Wall-clock duration of the run.
    pub fn seal(
        mut self,
        transcript: Option<serde_json::Value>,
        report: &ExecReport,
        outcome: &str,
        duration_ms: u64,
    ) {
        if let Some(evidence) = self.evidence.as_mut() {
            match transcript {
                Some(transcript) => evidence.write_json(evidence::TRANSCRIPT, &transcript),
                None => evidence.note(format!(
                    "{}: not written, no conversation was started",
                    evidence::TRANSCRIPT
                )),
            }
            evidence.write_json(evidence::EXEC, report);
        }
        // HACKATHON §15 "Agent: errors": the reason a run did not complete
        // is otherwise only in exec.json.
        if outcome != "completed" {
            telemetry::emit(TelemetryEvent::Error(event::Error {
                kind: outcome.to_string(),
                message: report.error.clone().unwrap_or_default(),
                recoverable: false,
                source: Some("exec".to_string()),
                origin_call_id: None,
            }));
        }
        telemetry::emit(TelemetryEvent::RunEnd(event::RunEnd {
            outcome: outcome.to_string(),
            duration_ms,
            dropped_events: telemetry::dropped_events(),
        }));
        telemetry::flush();
        if let Some(evidence) = self.evidence.as_mut() {
            evidence.write_report();
            evidence.write_manifest(&self.started_at, outcome);
        }
    }
}

/// Resolves with the signal's name when SIGINT or (on Unix) SIGTERM
/// arrives. Installing this replaces the default "terminate now" action for
/// the life of the future.
pub async fn shutdown_signal() -> &'static str {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut terminate = signal(SignalKind::terminate()).ok();
        tokio::select! {
            _ = tokio::signal::ctrl_c() => "SIGINT",
            _ = async {
                match terminate.as_mut() {
                    Some(terminate) => { terminate.recv().await; }
                    None => std::future::pending::<()>().await,
                }
            } => "SIGTERM",
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
        "SIGINT"
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
                    ViolationKind::TestConfigChanged => "test_config_changed",
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
