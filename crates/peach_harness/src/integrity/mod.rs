//! Test-integrity protection.
//!
//! Modifying, deleting, renaming, disabling or skipping a protected test is a
//! competition-rule violation that can disqualify the team (`HACKATHON.md` §8,
//! §31), and integrity is checked by comparing files before and after the run.
//! A prompt asking the model nicely is not enough for something with that
//! consequence, so this is enforced in the runtime (`CLAUDE.md` principle 3)
//! and verified afterwards.
//!
//! Three layers, in order of reliability:
//! 1. **Refuse** writes to protected paths at the tool layer, and shell
//!    commands that clearly mutate one.
//! 2. **Tell the model**, in plain text, which paths are protected, so a
//!    refusal is predictable rather than surprising (principle 4).
//! 3. **Verify** hashes after the run and restore anything that changed, so a
//!    mutation through some route nobody anticipated still leaves a clean
//!    repository.

mod globs;
mod manifest;
pub mod mixed;
mod shell_guard;

pub use globs::{DEFAULT_EXCLUDE_GLOBS, DEFAULT_PROTECTED_GLOBS, ProtectedSet};
pub use manifest::{FileState, IntegrityReport, Manifest, Violation, ViolationKind};
pub use shell_guard::{check_command, mutated_paths};

/// What a tool is trying to do to a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteOp {
    /// Create a file that does not exist yet.
    Create,
    /// Overwrite or patch an existing file.
    Modify,
    /// Delete a file.
    Remove,
}

impl WriteOp {
    fn describe(&self) -> &'static str {
        match self {
            WriteOp::Create => "create",
            WriteOp::Modify => "modify",
            WriteOp::Remove => "remove",
        }
    }
}

/// Refusal text for an operation on a protected path, or `None` when allowed.
///
/// The text is written for the model: it says what was refused, why, and what
/// to do instead, because a bare denial tends to produce retries of the same
/// action.
pub fn check_tool_path(
    protected: &ProtectedSet,
    op: WriteOp,
    path: &std::path::Path,
) -> Option<String> {
    if !protected.is_protected(path) {
        return None;
    }
    let shown = protected.display_path(path);
    Some(format!(
        "Refused: cannot {} `{}` because it is a protected test file.\n\
         The evaluation rules forbid modifying, deleting, renaming, disabling or skipping tests, \
         and the repository's test files are compared before and after this run.\n\
         The tests describe the behaviour that is wanted. Change the source code so the existing \
         tests pass, rather than changing the tests.",
        op.describe(),
        shown
    ))
}

/// Plain-text notice naming the mixed files whose test sections are watched,
/// for the model's context. `None` when there are none.
pub fn test_config_notice(manifest: &Manifest) -> Option<String> {
    let watched: Vec<&str> = manifest
        .test_sections
        .iter()
        .filter(|s| !s.canonical.is_empty())
        .map(|s| s.path.as_str())
        .collect();
    if watched.is_empty() {
        return None;
    }
    Some(format!(
        "TEST CONFIGURATION — the test sections of these files must not change: {}.\n\
         That means test scripts, test-runner settings (jest, pytest, tox) and test targets. \
         Other edits to these files are fine. Changes to their test sections are reported as \
         integrity violations after the run.",
        watched.join(", ")
    ))
}

/// Plain-text notice listing the protected paths, for the model's context.
///
/// Returns `None` when nothing is protected, so no notice is injected when the
/// repository has no tests to protect.
pub fn model_notice(protected: &ProtectedSet) -> Option<String> {
    let files = protected.protected_files();
    if files.is_empty() {
        return None;
    }

    let shown: Vec<String> = files.iter().take(20).map(|p| p.to_string()).collect();
    let remainder = files.len().saturating_sub(shown.len());
    let more = if remainder > 0 {
        format!("\n… and {remainder} more.")
    } else {
        String::new()
    };

    Some(format!(
        "PROTECTED TEST FILES — these are read-only for this task.\n\
         Attempts to write, patch, delete or rename them will be refused, and they are compared \
         before and after the run.\n\
         Use them to verify your work by running the test suite. Make the existing tests pass by \
         changing the source code.\n\n{}{}",
        shown.join("\n"),
        more
    ))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use pretty_assertions::assert_eq;

    use super::*;

    fn fixture_protected() -> ProtectedSet {
        ProtectedSet::for_test(
            PathBuf::from("/repo"),
            vec![PathBuf::from("/repo/tests/test_math.py")],
        )
    }

    #[test]
    fn test_writing_a_protected_path_is_refused_with_guidance() {
        let fixture = fixture_protected();

        let actual = check_tool_path(
            &fixture,
            WriteOp::Modify,
            &PathBuf::from("/repo/tests/test_math.py"),
        );

        let message = actual.expect("a protected path should be refused");
        assert!(message.contains("Refused: cannot modify `tests/test_math.py`"));
        assert!(message.contains("Change the source code"));
    }

    #[test]
    fn test_writing_an_unprotected_path_is_allowed() {
        let fixture = fixture_protected();

        let actual = check_tool_path(
            &fixture,
            WriteOp::Modify,
            &PathBuf::from("/repo/src/math.py"),
        );

        assert_eq!(actual, None);
    }

    #[test]
    fn test_notice_is_absent_when_nothing_is_protected() {
        let fixture = ProtectedSet::for_test(PathBuf::from("/repo"), vec![]);

        assert_eq!(model_notice(&fixture), None);
    }

    #[test]
    fn test_notice_names_the_protected_files() {
        let fixture = fixture_protected();

        let actual = model_notice(&fixture).expect("a notice should be produced");

        assert!(actual.contains("tests/test_math.py"));
        assert!(actual.contains("PROTECTED TEST FILES"));
    }
}

/// Lists every file under `root`, for the one place file discovery happens.
///
/// Uses `ignore::WalkBuilder`, which respects the repository's own
/// `.gitignore` in addition to our exclude globs — a file the repo itself
/// ignores is not a source of tests to protect. Symlinks are not followed, so
/// a symlink pointing outside the repo cannot be used to smuggle a test
/// file's manifest entry to an unexpected location.
fn walk_files(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let walker = ignore::WalkBuilder::new(root)
        .follow_links(false)
        .hidden(false)
        .build();
    walker
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_some_and(|t| t.is_file()))
        .map(ignore::DirEntry::into_path)
        .collect()
}

/// `files` relative to `root`, without the excluded trees (dependencies,
/// build output), whose `package.json` files are not the repository's own.
fn relative_paths(
    root: &std::path::Path,
    files: &[std::path::PathBuf],
    exclude_globs: &[String],
) -> Vec<String> {
    let excluded: Vec<glob::Pattern> = exclude_globs
        .iter()
        .filter_map(|g| glob::Pattern::new(g).ok())
        .collect();
    files
        .iter()
        .filter_map(|path| path.strip_prefix(root).ok())
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .filter(|relative| !excluded.iter().any(|pattern| pattern.matches(relative)))
        .collect()
}

/// Discovers protected files under `root` and captures a manifest of them,
/// starting the test-integrity guard for one run. `ProtectedSet::new` stays
/// pure and testable against an explicit file list; this is the only caller
/// that finds that list by walking a real repository.
pub fn discover_and_capture(
    root: &std::path::Path,
    protect_globs: &[String],
    exclude_globs: &[String],
    snapshot_dir: Option<&std::path::Path>,
) -> (ProtectedSet, Manifest) {
    let files = walk_files(root);
    let mut manifest = Manifest::capture(
        root,
        &ProtectedSet::new(root, protect_globs, exclude_globs, files.clone()).protected_files(),
        snapshot_dir,
    );
    let protected = ProtectedSet::new(root, protect_globs, exclude_globs, files.clone());
    manifest.test_sections = mixed::capture(root, &relative_paths(root, &files, exclude_globs));
    (protected, manifest)
}

/// Verifies the manifest against the repository's current state, restoring
/// any violation, and re-walks the repository first so a newly added test
/// file is included as a candidate violation (`ViolationKind::Added`) rather
/// than only files that existed when the manifest was captured.
pub fn verify_and_restore(
    root: &std::path::Path,
    protect_globs: &[String],
    exclude_globs: &[String],
    manifest: &Manifest,
) -> IntegrityReport {
    let now = ProtectedSet::new(root, protect_globs, exclude_globs, walk_files(root));
    let report = manifest.verify(&now.protected_files());
    let mut report = if report.is_clean() {
        report
    } else {
        manifest.restore(&report)
    };
    // D-054: test sections of mixed files are flagged, never restored.
    report.checked += manifest.test_sections.len();
    report.violations.extend(
        mixed::changed(root, &manifest.test_sections)
            .into_iter()
            .map(|path| Violation {
                path,
                kind: ViolationKind::TestConfigChanged,
                restored: false,
            }),
    );
    report
}

/// Telemetry events describing a post-run integrity check: one summary
/// `verify` event, then one event per violation — `restored` when the
/// original content was put back, `violation` when it could not be.
///
/// The summary is emitted even when the run was clean, so the log proves the
/// check ran rather than leaving "clean" and "never checked" looking the same.
pub fn telemetry_events(report: &IntegrityReport) -> Vec<crate::telemetry::TelemetryEvent> {
    use crate::telemetry::TelemetryEvent;
    use crate::telemetry::event::Integrity;

    let restored = report.violations.iter().filter(|v| v.restored).count();
    let summary = if report.is_clean() {
        format!("{} protected files checked; all unchanged", report.checked)
    } else {
        format!(
            "{} protected files checked; {} violation(s), {} restored",
            report.checked,
            report.violations.len(),
            restored
        )
    };

    std::iter::once(TelemetryEvent::Integrity(Integrity {
        kind: "verify".to_string(),
        path: None,
        detail: summary,
    }))
    .chain(report.violations.iter().map(|violation| {
        let kind = match violation.kind {
            ViolationKind::Modified => "modified",
            ViolationKind::Deleted => "deleted",
            ViolationKind::Added => "added",
            ViolationKind::TestConfigChanged => "test_config_changed",
        };
        let (event_kind, detail) = if violation.kind == ViolationKind::TestConfigChanged {
            (
                "violation",
                "its test configuration changed during the run; flagged, not restored".to_string(),
            )
        } else if violation.restored {
            (
                "restored",
                format!("{kind} during the run; restored to its pre-run state"),
            )
        } else {
            (
                "violation",
                format!("{kind} during the run; could NOT be restored"),
            )
        };
        TelemetryEvent::Integrity(Integrity {
            kind: event_kind.to_string(),
            path: Some(violation.path.clone()),
            detail,
        })
    }))
    .collect()
}

#[cfg(test)]
mod telemetry_event_tests {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::telemetry::TelemetryEvent;
    use crate::telemetry::event::Integrity;

    #[test]
    fn test_a_clean_check_still_records_that_it_ran() {
        let fixture = IntegrityReport { violations: vec![], checked: 3 };

        let actual = telemetry_events(&fixture);

        let expected = vec![TelemetryEvent::Integrity(Integrity {
            kind: "verify".to_string(),
            path: None,
            detail: "3 protected files checked; all unchanged".to_string(),
        })];
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_each_violation_is_its_own_event() {
        let fixture = IntegrityReport {
            violations: vec![
                Violation {
                    path: "tests/a.py".to_string(),
                    kind: ViolationKind::Modified,
                    restored: true,
                },
                Violation {
                    path: "tests/b.py".to_string(),
                    kind: ViolationKind::Deleted,
                    restored: false,
                },
            ],
            checked: 2,
        };

        let actual = telemetry_events(&fixture);

        let expected = vec![
            TelemetryEvent::Integrity(Integrity {
                kind: "verify".to_string(),
                path: None,
                detail: "2 protected files checked; 2 violation(s), 1 restored".to_string(),
            }),
            TelemetryEvent::Integrity(Integrity {
                kind: "restored".to_string(),
                path: Some("tests/a.py".to_string()),
                detail: "modified during the run; restored to its pre-run state".to_string(),
            }),
            TelemetryEvent::Integrity(Integrity {
                kind: "violation".to_string(),
                path: Some("tests/b.py".to_string()),
                detail: "deleted during the run; could NOT be restored".to_string(),
            }),
        ];
        assert_eq!(actual, expected);
    }
}
