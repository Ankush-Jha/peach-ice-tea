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
mod shell_guard;

pub use globs::{DEFAULT_EXCLUDE_GLOBS, DEFAULT_PROTECTED_GLOBS, ProtectedSet};
pub use manifest::{FileState, IntegrityReport, Manifest, Violation, ViolationKind};
pub use shell_guard::check_command;

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
pub fn check_tool_path(protected: &ProtectedSet, op: WriteOp, path: &std::path::Path) -> Option<String> {
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

        let actual = check_tool_path(&fixture, WriteOp::Modify, &PathBuf::from("/repo/src/math.py"));

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
