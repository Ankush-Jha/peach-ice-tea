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
//!
//! `is_non_interactive` has a second, weaker source: an environment variable
//! (`NON_INTERACTIVE_ENV_VAR`). `peach exec` (`crates/peach_main`) is the one
//! caller that needs to flip this on before a human could possibly be asked
//! anything, but `peach_main` does not depend on this crate and adding that
//! dependency is a `Cargo.toml` edit outside this piece of work (TH.1
//! report). The env var is a same-process, no-new-dependency stand-in for
//! calling [`install`] directly; a real installed [`HarnessRuntime`] always
//! takes priority when both are present.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::integrity::ProtectedSet;

static RUNTIME: OnceLock<HarnessRuntime> = OnceLock::new();

/// Environment variable that marks the process as an unattended harness run
/// when no [`HarnessRuntime`] has been installed. See the module docs for why
/// this exists alongside [`install`].
pub const NON_INTERACTIVE_ENV_VAR: &str = "PEACH_HARNESS_NON_INTERACTIVE";

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
///
/// True when a [`HarnessRuntime`] was installed with `non_interactive(true)`,
/// or — failing that — when [`NON_INTERACTIVE_ENV_VAR`] is set to `"1"` (see
/// the module docs).
pub fn is_non_interactive() -> bool {
    get().is_some_and(|runtime| runtime.non_interactive)
        || env_flag_is_set(std::env::var(NON_INTERACTIVE_ENV_VAR).ok())
}

/// Whether an environment variable value counts as "set" for
/// [`is_non_interactive`]'s env-var fallback.
///
/// Kept as a pure function of the value rather than reading `std::env`
/// directly so it can be unit tested without mutating real process state,
/// which would race with any other test in this binary reading the same
/// variable.
fn env_flag_is_set(value: Option<String>) -> bool {
    value.is_some_and(|v| v == "1")
}

/// Refusal text if this operation targets a protected test file.
///
/// Answers `None` whenever no runtime is installed, so behaviour outside a
/// harness run is unchanged.
pub fn check_write(op: crate::integrity::WriteOp, path: &Path) -> Option<String> {
    let runtime = get()?;
    let protected = runtime.protected_set()?;
    let refusal = crate::integrity::check_tool_path(protected, op, path)?;
    record_refusal(Some(protected.display_path(path)), &refusal);
    Some(refusal)
}

/// Refusal text if this shell command would mutate a protected test file.
pub fn check_shell(command: &str) -> Option<String> {
    let runtime = get()?;
    let protected = runtime.protected_set()?;
    let refusal = crate::integrity::check_command(protected, command, runtime.repo_root())?;
    record_refusal(None, &refusal);
    Some(refusal)
}

/// Records a refusal in telemetry. A refused attempt on a test is exactly
/// what a judge reviewing test integrity wants to see (HACKATHON.md §8), and
/// the tool result the model receives is not part of the telemetry stream.
fn record_refusal(path: Option<String>, refusal: &str) {
    crate::telemetry::emit(crate::telemetry::TelemetryEvent::Integrity(
        crate::telemetry::event::Integrity {
            kind: "refused".to_string(),
            path,
            detail: refusal.lines().next().unwrap_or_default().to_string(),
        },
    ));
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
    fn test_env_flag_is_set_only_for_the_exact_value() {
        assert!(env_flag_is_set(Some("1".to_string())));
        assert!(!env_flag_is_set(Some("0".to_string())));
        assert!(!env_flag_is_set(Some("true".to_string())));
        assert!(!env_flag_is_set(Some(String::new())));
        assert!(!env_flag_is_set(None));
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
