//! Process-wide state for one harness run.
//!
//! The integrity guard has to be consulted deep inside tool dispatch, where
//! threading new state through would mean changing generic bounds on upstream
//! services and every implementation of them — a large, merge-hostile diff for
//! something that is genuinely per-process (`DECISIONS.md` D-004). A run is one
//! process, so this is installed once at start-up and read from wherever it is
//! needed.
//!
//! Nothing is installed by default. Every accessor answers "not active" when
//! there is no runtime, so interactive use and existing tests behave exactly as
//! before.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::integrity::ProtectedSet;

static RUNTIME: OnceLock<HarnessRuntime> = OnceLock::new();

/// State shared by the harness subsystems for the duration of a run.
#[derive(Debug)]
pub struct HarnessRuntime {
    /// Whether this run can ever ask a human a question.
    non_interactive: bool,
    /// Repository the task operates on.
    repo_root: PathBuf,
    /// Test files the run must not modify, when the guard is active.
    protected: Option<ProtectedSet>,
}

impl HarnessRuntime {
    /// Builds a runtime for a run.
    pub fn new(repo_root: impl Into<PathBuf>) -> Self {
        Self { non_interactive: false, repo_root: repo_root.into(), protected: None }
    }

    /// Marks the run as unattended, so nothing may prompt for input.
    pub fn non_interactive(mut self, value: bool) -> Self {
        self.non_interactive = value;
        self
    }

    /// Activates the test-integrity guard with this protected set.
    pub fn protected(mut self, protected: ProtectedSet) -> Self {
        self.protected = Some(protected);
        self
    }

    /// Repository root for this run.
    pub fn repo_root(&self) -> &Path {
        &self.repo_root
    }

    /// Protected test files, when the guard is active.
    pub fn protected_set(&self) -> Option<&ProtectedSet> {
        self.protected.as_ref()
    }
}

/// Installs the runtime for this process.
///
/// Returns `false` if one was already installed, which is not treated as an
/// error: the first installation wins, and a second one means the caller ran
/// two sessions in one process.
pub fn install(runtime: HarnessRuntime) -> bool {
    RUNTIME.set(runtime).is_ok()
}

/// The installed runtime, if any.
pub fn get() -> Option<&'static HarnessRuntime> {
    RUNTIME.get()
}

/// Whether this process is an unattended harness run.
pub fn is_non_interactive() -> bool {
    get().is_some_and(|runtime| runtime.non_interactive)
}

/// Refusal text if this operation targets a protected test file.
///
/// Answers `None` whenever no runtime is installed, so behaviour outside a
/// harness run is unchanged.
pub fn check_write(op: crate::integrity::WriteOp, path: &Path) -> Option<String> {
    let runtime = get()?;
    let protected = runtime.protected_set()?;
    crate::integrity::check_tool_path(protected, op, path)
}

/// Refusal text if this shell command would mutate a protected test file.
pub fn check_shell(command: &str) -> Option<String> {
    let runtime = get()?;
    let protected = runtime.protected_set()?;
    crate::integrity::check_command(protected, command, runtime.repo_root())
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_nothing_is_guarded_when_no_runtime_is_installed() {
        // This process installs no runtime, so every accessor must be inert.
        assert_eq!(check_shell("rm tests/test_math.py"), None);
        assert_eq!(
            check_write(crate::integrity::WriteOp::Modify, Path::new("/repo/tests/test_math.py")),
            None
        );
        assert!(!is_non_interactive());
    }

    #[test]
    fn test_a_runtime_carries_its_protected_set() {
        let fixture = HarnessRuntime::new("/repo")
            .non_interactive(true)
            .protected(ProtectedSet::for_test(
                PathBuf::from("/repo"),
                vec![PathBuf::from("/repo/tests/test_math.py")],
            ));

        assert!(fixture.protected_set().is_some());
        assert_eq!(fixture.repo_root(), Path::new("/repo"));
    }
}
