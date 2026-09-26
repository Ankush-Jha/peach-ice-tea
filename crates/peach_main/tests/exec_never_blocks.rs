//! harness: R-HACK-1 — proves `peach exec` reaches a conclusion within a
//! bounded time in a one-shot, unattended run, whether stdin is closed or
//! left open on a pipe the parent never writes to or closes. The second shape
//! is exactly how an automated runner spawns a subprocess (see the stdin-skip
//! comment in `crates/peach_main/src/main.rs`) and is the scenario that used
//! to hang before that fix.
//!
//! No scenario here reaches a successful model call — the project's eval
//! budget is ₹100 total (D-025) and none of it is spent here. Each run is
//! configured to fail before or shortly after its first attempt to reach a
//! provider:
//!
//! - `missing_model_config`: nothing is configured at all.
//! - `unknown_provider`: a provider id that exists nowhere in the registry.
//! - `provider_connection_failure`: a real, configured provider whose URL
//!   points at a closed local port, capped with `--max-duration-secs` — a
//!   real run pays for `RetryConfig`'s backoff every time a request fails at
//!   the transport level, and that backoff is not itself bounded (confirmed
//!   manually: an uncapped run here was still retrying past two minutes
//!   before being killed), so this scenario is the one place a wall-clock
//!   budget is not optional for a bounded test.
//!
//! Every test uses its own `PEACH_CONFIG` directory (`tempfile::tempdir`), so
//! `~/.peach` is never read or written.

use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use tempfile::TempDir;

/// Upper bound every scenario below must finish well inside of. Measured
/// wall-clock times (see the TH.1 report) are a few hundred milliseconds for
/// `missing_model_config` and `unknown_provider`, and a few seconds over the
/// requested budget for `provider_connection_failure`.
const BOUND: Duration = Duration::from_secs(25);

fn peach_bin() -> &'static str {
    env!("CARGO_BIN_EXE_peach")
}

/// A fresh, empty `PEACH_CONFIG` directory, so a test run never touches
/// `~/.peach` (`crates/peach_config/src/reader.rs::resolve_base_path`).
fn isolated_config_dir() -> TempDir {
    tempfile::tempdir().expect("create isolated PEACH_CONFIG dir")
}

fn isolated_project_dir() -> TempDir {
    tempfile::tempdir().expect("create isolated project dir")
}

/// Polls `child` for exit without reading its stdout, killing and panicking —
/// the exact hang this file exists to catch — if `bound` elapses first.
fn wait_bounded(mut child: Child, bound: Duration) -> (std::process::ExitStatus, Duration) {
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait().expect("try_wait") {
            return (status, started.elapsed());
        }
        if started.elapsed() > bound {
            let _ = child.kill();
            let _ = child.wait();
            panic!(
                "peach exec did not exit within {bound:?} — this is exactly the hang R-HACK-1 \
                 forbids"
            );
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Last line of `output` that parses as JSON — where `exec --json` prints
/// its report (`ExecReport::to_json_line` or the hand-built time-budget
/// report both print a single JSON line as the final line of stdout).
fn last_json_line(output: &str) -> serde_json::Value {
    output
        .lines()
        .rev()
        .find_map(|line| serde_json::from_str::<serde_json::Value>(line.trim()).ok())
        .unwrap_or_else(|| panic!("no JSON line found in output:\n{output}"))
}

/// Runs `peach exec` with stdin closed and stdout captured, waiting at most
/// `BOUND` for it to exit.
fn run_with_closed_stdin(
    config_dir: &TempDir,
    project_dir: &TempDir,
    extra_env: &[(&str, &str)],
    args: &[&str],
) -> (std::process::ExitStatus, String, Duration) {
    let mut command = Command::new(peach_bin());
    command
        .args(args)
        .env("PEACH_CONFIG", config_dir.path())
        .current_dir(project_dir.path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    for (key, value) in extra_env {
        command.env(key, value);
    }

    let mut child = command.spawn().expect("spawn peach exec");
    let mut stdout = child.stdout.take();
    let (status, elapsed) = wait_bounded(child, BOUND);

    let mut output = String::new();
    if let Some(stdout) = stdout.as_mut() {
        let _ = stdout.read_to_string(&mut output);
    }
    (status, output, elapsed)
}

/// Runs `peach exec` with stdin as the read end of a pipe whose write end
/// this process holds open for the whole run — never written to, never
/// closed until after the process has already exited. This is the shape a
/// harness runner uses (open a pipe to the child, keep the handle for later,
/// never send EOF), and is the case that used to hang before the stdin-skip
/// fix in `main.rs`.
fn run_with_open_never_written_pipe(
    config_dir: &TempDir,
    project_dir: &TempDir,
    extra_env: &[(&str, &str)],
    args: &[&str],
) -> (std::process::ExitStatus, Duration) {
    let mut command = Command::new(peach_bin());
    command
        .args(args)
        .env("PEACH_CONFIG", config_dir.path())
        .current_dir(project_dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    for (key, value) in extra_env {
        command.env(key, value);
    }

    let mut child = command.spawn().expect("spawn peach exec");
    // Deliberately not dropped until after `wait_bounded` returns: dropping
    // `ChildStdin` closes the pipe (sends EOF), which would test the wrong
    // thing. Held here so the write end stays open across the whole wait.
    let held_stdin = child.stdin.take();

    let (status, elapsed) = wait_bounded(child, BOUND);
    drop(held_stdin);
    (status, elapsed)
}

// ---- Scenario 1: missing model configuration -------------------------------
//
// A completely fresh config directory: no `.peach.toml` session, no
// `PEACH_SESSION__*` env vars. `init_state_exec` (`ui.rs`) fails on its first
// check, before `on_message` ever runs.

#[test]
fn test_missing_model_config_exits_fast_with_closed_stdin() {
    let config = isolated_config_dir();
    let project = isolated_project_dir();

    let (status, output, elapsed) =
        run_with_closed_stdin(&config, &project, &[], &["exec", "do nothing", "--json"]);

    assert_eq!(status.code(), Some(1), "output:\n{output}");
    let report = last_json_line(&output);
    assert_eq!(report["outcome"], "error");
    assert_eq!(report["exit_code"], 1);
    assert!(
        report["error"]
            .as_str()
            .unwrap_or_default()
            .contains("No model or provider is configured")
    );
    assert!(elapsed < BOUND);
}

#[test]
fn test_missing_model_config_exits_fast_with_open_never_written_pipe() {
    let config = isolated_config_dir();
    let project = isolated_project_dir();

    let (status, elapsed) = run_with_open_never_written_pipe(
        &config,
        &project,
        &[],
        &["exec", "do nothing", "--json"],
    );

    assert_eq!(status.code(), Some(1));
    assert!(elapsed < BOUND);
}

// ---- Scenario 2: unknown provider ------------------------------------------
//
// A session pointed at a provider id that exists nowhere in the built-in or
// configured provider list. Fails locally, before any network attempt, once
// `exec` tries to resolve the provider for the first request.

#[test]
fn test_unknown_provider_exits_fast_with_closed_stdin() {
    let config = isolated_config_dir();
    let project = isolated_project_dir();
    let env = [
        ("PEACH_SESSION__PROVIDER_ID", "totally-unknown-provider-xyz"),
        ("PEACH_SESSION__MODEL_ID", "whatever-model"),
    ];

    let (status, output, elapsed) =
        run_with_closed_stdin(&config, &project, &env, &["exec", "do nothing", "--json"]);

    assert_eq!(status.code(), Some(1), "output:\n{output}");
    let report = last_json_line(&output);
    assert_eq!(report["outcome"], "error");
    assert_eq!(report["exit_code"], 1);
    assert!(elapsed < BOUND);
}

#[test]
fn test_unknown_provider_exits_fast_with_open_never_written_pipe() {
    let config = isolated_config_dir();
    let project = isolated_project_dir();
    let env = [
        ("PEACH_SESSION__PROVIDER_ID", "totally-unknown-provider-xyz"),
        ("PEACH_SESSION__MODEL_ID", "whatever-model"),
    ];

    let (status, elapsed) = run_with_open_never_written_pipe(
        &config,
        &project,
        &env,
        &["exec", "do nothing", "--json"],
    );

    assert_eq!(status.code(), Some(1));
    assert!(elapsed < BOUND);
}

// ---- Scenario 3: a normal run that errors at the provider ------------------
//
// A provider that *is* configured and recognised (defined inline in
// `.peach.toml`, credentialed via a bogus env var so migration and rendering
// both succeed) but whose URL is a closed local port, so the first request
// fails with a transport-level "connection refused". `RetryConfig` then
// backs off and retries; `--max-duration-secs` is what makes this scenario
// finish at all inside a bounded test — see the module docs.

fn write_unroutable_provider_config(config_dir: &TempDir) {
    std::fs::write(
        config_dir.path().join(".peach.toml"),
        r#"
[[providers]]
id = "test_unroutable"
url = "http://127.0.0.1:1/v1/chat/completions"
response_type = "OpenAI"
auth_methods = ["api_key"]
api_key_var = "PEACH_TEST_BOGUS_KEY"

[[providers.models]]
id = "test-model"
name = "Test Model"
input_modalities = ["text"]

[session]
provider_id = "test_unroutable"
model_id = "test-model"
"#,
    )
    .expect("write isolated .peach.toml");
}

#[test]
fn test_provider_connection_failure_exits_fast_with_closed_stdin() {
    let config = isolated_config_dir();
    let project = isolated_project_dir();
    write_unroutable_provider_config(&config);
    let env = [("PEACH_TEST_BOGUS_KEY", "bogus-key-value")];

    let (status, output, elapsed) = run_with_closed_stdin(
        &config,
        &project,
        &env,
        &["exec", "do nothing", "--json", "--max-duration-secs", "3"],
    );

    assert_eq!(status.code(), Some(4), "output:\n{output}");
    let report = last_json_line(&output);
    assert_eq!(report["outcome"], "time_budget");
    assert_eq!(report["exit_code"], 4);
    assert!(elapsed < BOUND);
}

#[test]
fn test_provider_connection_failure_exits_fast_with_open_never_written_pipe() {
    let config = isolated_config_dir();
    let project = isolated_project_dir();
    write_unroutable_provider_config(&config);
    let env = [("PEACH_TEST_BOGUS_KEY", "bogus-key-value")];

    let (status, elapsed) = run_with_open_never_written_pipe(
        &config,
        &project,
        &env,
        &["exec", "do nothing", "--json", "--max-duration-secs", "3"],
    );

    assert_eq!(status.code(), Some(4));
    assert!(elapsed < BOUND);
}

// ---- Scenario 4: a project MCP config under a real terminal ----------------
//
// peach asks whether to trust a project's `.mcp.json`. Under a TTY that
// prompt waits for a keypress: reproduced with `script`, the run sat on
// "Accept / Reject" until its budget with no model call. An unattended run
// must refuse without asking.

#[cfg(target_os = "macos")]
#[test]
fn test_a_project_mcp_config_never_prompts_even_with_a_terminal() {
    let config = isolated_config_dir();
    let project = isolated_project_dir();
    write_unroutable_provider_config(&config);
    std::fs::write(
        project.path().join(".mcp.json"),
        r#"{"mcpServers":{"local":{"command":"sleep","args":["60"]}}}"#,
    )
    .unwrap();
    let transcript = project.path().join("tty.log");

    // `script` gives peach a pseudo-terminal, as a judge's shell would.
    let mut command = Command::new("script");
    command
        .arg("-q")
        .arg(&transcript)
        .arg(peach_bin())
        .args(["exec", "do nothing", "--json", "--max-duration-secs", "6"])
        .env("PEACH_CONFIG", config.path())
        .env("PEACH_TEST_BOGUS_KEY", "bogus-key-value")
        .current_dir(project.path())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let child = command.spawn().expect("spawn script");
    let (_status, elapsed) = wait_bounded(child, BOUND);

    let output = std::fs::read_to_string(&transcript).unwrap_or_default();
    assert!(!output.contains("Accept"), "the trust prompt was shown:\n{output}");
    // It reached the (closed-port) provider instead of waiting: transport
    // retries, then the budget, not a prompt. A pty merges stderr into the
    // transcript, so the spinner's last frame shares the JSON's line.
    let json = &output[output.rfind("{\"outcome\"").expect("no outcome JSON in transcript")..];
    let report = last_json_line(json);
    assert_eq!(report["outcome"], "time_budget");
    assert!(elapsed < BOUND);
}

