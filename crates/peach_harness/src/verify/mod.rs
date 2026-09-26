//! Verified completion (R-HACK-7, HACKATHON.md §12, §23).
//!
//! Three things, all active only in an unattended `exec` run:
//! - every test run the agent makes is classified and recorded
//!   ([`record_test_run`]);
//! - a run may not end on an unverified edit: when the agent stops after
//!   editing source with no passing test run since, it is told to verify,
//!   at most [`MAX_GATE_NUDGES`] times ([`gate_message`]);
//! - after the agent stops, the harness runs the tests once itself
//!   ([`run_final`]), so the evidence says whether they pass rather than
//!   whether the agent claimed they did (§16).

mod classify;
mod detect;

use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub use classify::{Classification, FailureClass, classify};
pub use detect::{TestCommand, detect, is_test_command};
use serde::{Deserialize, Serialize};

use crate::telemetry::{self, TelemetryEvent, event};

/// How many times the gate asks the agent to verify before giving up and
/// recording the run as unverified. Two, as PLAN.md W2-D set: enough to
/// recover from a forgotten test run, bounded so the gate cannot loop.
pub const MAX_GATE_NUDGES: u32 = 2;

/// Environment variable that disables the completion gate when set to `0`.
pub const GATE_ENV_VAR: &str = "PEACH_HARNESS_VERIFY_GATE";

/// Environment variable that disables recovery hints when set to `0`.
pub const HINTS_ENV_VAR: &str = "PEACH_HARNESS_RECOVERY_HINTS";

#[derive(Debug, Default)]
struct State {
    edited: bool,
    edited_since_green: bool,
    nudges: u32,
    gave_up: bool,
}

static STATE: Mutex<State> = Mutex::new(State {
    edited: false,
    edited_since_green: false,
    nudges: 0,
    gave_up: false,
});

/// Records a successful source edit (write, patch, remove, undo).
pub fn record_edit() {
    if let Ok(mut state) = STATE.lock() {
        state.edited = true;
        state.edited_since_green = true;
    }
}

/// Records a shell command. When it runs the tests, classifies the result,
/// emits `test_run`, and clears the gate if it passed. Returns a recovery
/// hint to append to the tool result when the failure happened before the
/// agent's code was exercised (see [`recovery_hint`]).
///
/// # Arguments
/// * `command` - The shell command line.
/// * `exit_code` - Its exit code, when it exited on its own.
/// * `output` - stdout and stderr together.
/// * `duration_ms` - How long it took.
pub fn record_shell(command: &str, exit_code: Option<i32>, output: &str, duration_ms: u64) -> Option<String> {
    let detected = crate::runtime::get().and_then(|runtime| runtime.test_command());
    if !is_test_command(command, detected) {
        // D-037 gap closed: an edit made through the shell arms the gate too.
        if let Some(runtime) = crate::runtime::get()
            && exit_code == Some(0)
            && writes_inside(command, runtime.repo_root())
        {
            record_edit();
        }
        return None;
    }
    let classification = classify(exit_code, false, output);
    if classification.class == FailureClass::Passed
        && let Ok(mut state) = STATE.lock()
    {
        state.edited_since_green = false;
    }
    emit_test_run(command, exit_code, &classification, duration_ms, "agent");

    let unattended = crate::runtime::get().is_some_and(|runtime| runtime.is_non_interactive());
    if !unattended || std::env::var(HINTS_ENV_VAR).is_ok_and(|v| v == "0") {
        return None;
    }
    let hint = recovery_hint(classification.class, detected)?;
    telemetry::emit(TelemetryEvent::Recovery(event::Recovery {
        action: "recovery_hint".to_string(),
        trigger: classification.class.as_str().to_string(),
        outcome: None,
        origin_call_id: None,
    }));
    Some(hint)
}

/// One plain sentence naming what kind of failure this was, when that is
/// not already obvious from a normal failing-assertion output: a run that
/// failed before the agent's code was exercised is easy to misread as "my
/// fix is wrong". Never suggests touching tests. `None` for passes and
/// ordinary assertion failures, whose output speaks for itself — a hint
/// there would only cost tokens.
///
/// # Arguments
/// * `class` - The run's classification.
/// * `test` - The repository's test command, when known.
pub fn recovery_hint(class: FailureClass, test: Option<&TestCommand>) -> Option<String> {
    let command = test.map_or(String::new(), |test| format!(" The expected test command is `{}`.", test.command));
    match class {
        FailureClass::Environment => Some(format!(
            "RECOVERY HINT (harness): this run failed before any test executed — a runner, tool or module is \
             missing. That is an environment problem, not evidence against your code change.{command} \
             Use a runner that is already installed; do not install packages and do not modify tests."
        )),
        FailureClass::Compile => Some(
            "RECOVERY HINT (harness): the code did not compile or import, so no test result is meaningful \
             yet. Fix the first error shown above in the source code, then run the tests again."
                .to_string(),
        ),
        FailureClass::Timeout => Some(format!(
            "RECOVERY HINT (harness): the test run did not finish in time. Look for an infinite loop or \
             blocking call in the code you changed, or run a narrower selection of tests first.{command}"
        )),
        FailureClass::Passed | FailureClass::TestAssertion | FailureClass::Unknown => None,
    }
}

/// Whether `command` writes or deletes a path inside `repo` (redirects to
/// `/dev/null` or scratch files elsewhere don't count). Relative paths are
/// taken as relative to the repository root, where the agent's shell runs.
fn writes_inside(command: &str, repo: &Path) -> bool {
    crate::integrity::mutated_paths(command).iter().any(|target| {
        let target = target.trim_matches(['"', '\'']);
        let path = Path::new(target);
        if path.is_absolute() {
            path.starts_with(repo)
        } else {
            !target.is_empty() && !target.starts_with('&')
        }
    })
}

fn emit_test_run(command: &str, exit_code: Option<i32>, result: &Classification, duration_ms: u64, origin: &str) {
    telemetry::emit(TelemetryEvent::TestRun(event::TestRun {
        command: command.to_string(),
        exit_code,
        passed: result.passed,
        failed: result.failed,
        skipped: result.skipped,
        duration_ms,
        failure_class: Some(result.class.as_str().to_string()),
        origin: Some(origin.to_string()),
    }));
}

/// The message to send when the agent is about to finish, or `None` to let
/// it. Only in an unattended run, and never more than [`MAX_GATE_NUDGES`]
/// times; after that the run is recorded as `verification_unconfirmed` and
/// allowed to end.
pub fn gate_message() -> Option<String> {
    let runtime = crate::runtime::get()?;
    if !runtime.is_non_interactive() || std::env::var(GATE_ENV_VAR).is_ok_and(|v| v == "0") {
        return None;
    }
    let mut state = STATE.lock().ok()?;
    let (message, give_up) = gate_decision(&mut state, runtime.test_command());
    drop(state);
    if give_up {
        telemetry::emit(TelemetryEvent::AgentState(event::AgentState {
            from: None,
            to: "verification_unconfirmed".to_string(),
            reason: Some(format!(
                "source was edited after the last passing test run; {MAX_GATE_NUDGES} reminders were not acted on"
            )),
            iteration: None,
        }));
    }
    message
}

/// The gate's decision for `state`: the reminder to send, and whether this
/// call is the one that gives up. Pure apart from `state`, for testing.
fn gate_decision(state: &mut State, test: Option<&TestCommand>) -> (Option<String>, bool) {
    if !state.edited || !state.edited_since_green || state.gave_up {
        return (None, false);
    }
    if state.nudges >= MAX_GATE_NUDGES {
        state.gave_up = true;
        return (None, true);
    }
    state.nudges += 1;
    let how = match test {
        Some(test) => format!("Run `{}`", test.command),
        None => "Run the repository's test suite".to_string(),
    };
    (
        Some(format!(
            "VERIFICATION REQUIRED before finishing ({} of {MAX_GATE_NUDGES}): you changed source files \
             and have not run the tests successfully since. {how} and make it pass by fixing the source \
             code, never the tests. If the tests cannot run in this environment, run them anyway, then \
             say exactly why they could not pass in your final message.",
            state.nudges
        )),
        false,
    )
}

/// The harness's own test run after the agent stopped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinalRun {
    pub ran: bool,
    pub command: String,
    /// Why this command: `explicit` or the detection marker.
    pub source: String,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub duration_ms: u64,
    pub class: FailureClass,
    pub passed: Option<u64>,
    pub failed: Option<u64>,
    pub skipped: Option<u64>,
    /// The last lines of output, for the reviewer.
    pub output_tail: String,
}

/// Runs `test` in `root` with stdin closed, killing its whole process group
/// after `timeout`, then classifies and records the result.
///
/// # Arguments
/// * `root` - Repository root, the working directory.
/// * `test` - The command to run.
/// * `timeout` - Wall-clock limit.
pub fn run_final(root: &Path, test: &TestCommand, timeout: Duration) -> FinalRun {
    let started = Instant::now();
    let (exit_code, timed_out, output) = run_bounded(root, &test.command, timeout);
    let duration_ms = started.elapsed().as_millis() as u64;
    let result = classify(exit_code, timed_out, &output);
    emit_test_run(&test.command, exit_code, &result, duration_ms, "harness_final");
    let tail: Vec<&str> = output.lines().rev().take(40).collect();
    FinalRun {
        ran: true,
        command: test.command.clone(),
        source: test.source.clone(),
        exit_code,
        timed_out,
        duration_ms,
        class: result.class,
        passed: result.passed,
        failed: result.failed,
        skipped: result.skipped,
        output_tail: tail.into_iter().rev().collect::<Vec<_>>().join("\n"),
    }
}

fn run_bounded(root: &Path, command: &str, timeout: Duration) -> (Option<i32>, bool, String) {
    use std::io::Read;
    #[cfg(unix)]
    use std::os::unix::process::CommandExt;

    let mut builder = std::process::Command::new("/bin/sh");
    builder
        .args(["-c", &format!("{command} 2>&1")])
        .current_dir(root)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    #[cfg(unix)]
    builder.process_group(0);
    let mut child = match builder.spawn() {
        Ok(child) => child,
        Err(error) => return (None, false, format!("could not start the test command: {error}")),
    };
    let mut stdout = child.stdout.take();
    let reader = std::thread::spawn(move || {
        let mut buffer = Vec::new();
        if let Some(stdout) = stdout.as_mut() {
            let _ = stdout.read_to_end(&mut buffer);
        }
        buffer
    });
    let deadline = Instant::now() + timeout;
    let (exit_code, timed_out) = loop {
        match child.try_wait() {
            Ok(Some(status)) => break (status.code(), false),
            Ok(None) if Instant::now() >= deadline => {
                #[cfg(unix)]
                let _ = std::process::Command::new("kill")
                    .args(["-KILL", &format!("-{}", child.id())])
                    .status();
                let _ = child.kill();
                let _ = child.wait();
                break (None, true);
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(_) => break (None, false),
        }
    };
    let output = reader.join().unwrap_or_default();
    (exit_code, timed_out, bstr::ByteSlice::to_str_lossy(output.as_slice()).into_owned())
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_the_gate_nudges_twice_then_gives_up_once() {
        let mut fixture = State { edited: true, edited_since_green: true, ..State::default() };
        let test = TestCommand { command: "cargo test".into(), source: "explicit".into() };

        let actual: Vec<(bool, bool)> = (0..4)
            .map(|_| {
                let (message, give_up) = gate_decision(&mut fixture, Some(&test));
                (message.is_some(), give_up)
            })
            .collect();

        assert_eq!(actual, vec![(true, false), (true, false), (false, true), (false, false)]);
    }

    #[test]
    fn test_hints_only_for_failures_that_happened_before_the_code_ran() {
        let test = TestCommand { command: "python3 -m unittest".into(), source: "explicit".into() };

        let actual: Vec<bool> = [
            FailureClass::Passed,
            FailureClass::TestAssertion,
            FailureClass::Unknown,
            FailureClass::Environment,
            FailureClass::Compile,
            FailureClass::Timeout,
        ]
        .into_iter()
        .map(|class| recovery_hint(class, Some(&test)).is_some())
        .collect();

        assert_eq!(actual, vec![false, false, false, true, true, true]);
        let environment = recovery_hint(FailureClass::Environment, Some(&test)).unwrap();
        assert!(environment.contains("`python3 -m unittest`") && environment.contains("do not modify tests"));
    }

    #[test]
    fn test_shell_writes_inside_the_repo_count_as_edits() {
        let repo = Path::new("/repo");

        let actual: Vec<bool> = [
            "sed -i 's/-/+/' calc.py",
            "cat > /repo/calc.py <<EOF",
            "echo x > /dev/null",
            "python3 -c 'print(1)' > /tmp/out.txt",
            "ls -la",
            "cat calc.py 2>&1",
        ]
        .iter()
        .map(|command| writes_inside(command, repo))
        .collect();

        assert_eq!(actual, vec![true, true, false, false, false, false]);
    }

    #[test]
    fn test_the_gate_is_quiet_without_an_unverified_edit() {
        let mut no_edit = State::default();
        let mut verified = State { edited: true, edited_since_green: false, ..State::default() };

        let actual = (gate_decision(&mut no_edit, None), gate_decision(&mut verified, None));

        assert_eq!(actual, ((None, false), (None, false)));
    }

    #[test]
    fn test_the_reminder_names_the_command_and_forbids_editing_tests() {
        let mut fixture = State { edited: true, edited_since_green: true, ..State::default() };
        let test = TestCommand { command: "npm test".into(), source: "explicit".into() };

        let (message, _) = gate_decision(&mut fixture, Some(&test));

        let message = message.unwrap();
        assert!(message.contains("Run `npm test`"));
        assert!(message.contains("never the tests"));
    }

    #[cfg(unix)]
    #[test]
    fn test_the_final_run_is_bounded_and_classified() {
        let dir = tempfile::tempdir().unwrap();
        let passing = TestCommand { command: "echo 'Ran 2 tests in 0.1s'; echo; echo OK".into(), source: "explicit".into() };
        let hanging = TestCommand { command: "sleep 30".into(), source: "explicit".into() };

        let ok = run_final(dir.path(), &passing, Duration::from_secs(10));
        let started = Instant::now();
        let hung = run_final(dir.path(), &hanging, Duration::from_millis(300));

        assert_eq!((ok.class, ok.passed, ok.failed), (FailureClass::Passed, Some(2), Some(0)));
        assert_eq!((hung.class, hung.timed_out), (FailureClass::Timeout, true));
        assert!(started.elapsed() < Duration::from_secs(5), "the timeout must kill the process");
    }
}
