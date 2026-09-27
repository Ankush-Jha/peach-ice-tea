//! harness: R-HACK-2 / D-028 — proves the post-run test-integrity check runs
//! inside the real `peach exec` binary, not just in `peach_harness` in
//! isolation.
//!
//! No model is involved (₹100 total budget, D-025): the provider points at a
//! closed local port, so the run spends its `--max-duration-secs` budget in
//! transport retries (the same setup as `exec_never_blocks.rs`). While it is
//! alive, this test plays a misbehaving agent: once the harness has written
//! `run_start` to its telemetry — the point after which the manifest exists —
//! it overwrites a protected test and adds a new one. The binary must report
//! both in its JSON line, put the repository back, and log it in telemetry.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use pretty_assertions::assert_eq;

const BOUND: Duration = Duration::from_secs(30);
const ORIGINAL_TEST: &str = "def test_add():\n    assert 1 + 1 == 2\n";

fn write_unroutable_provider_config(config_dir: &Path) {
    std::fs::write(
        config_dir.join(".peach.toml"),
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
    .unwrap();
}

fn wait_for_run_start(telemetry: &Path) {
    let started = Instant::now();
    while started.elapsed() < BOUND {
        let text = std::fs::read_to_string(telemetry).unwrap_or_default();
        if text.contains(r#""type":"run_start""#) {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!(
        "peach exec never wrote run_start to {}",
        telemetry.display()
    );
}

#[test]
fn test_a_test_edited_mid_run_is_reported_restored_and_logged() {
    let config = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let evidence = tempfile::tempdir().unwrap();
    write_unroutable_provider_config(config.path());
    std::fs::create_dir_all(project.path().join("tests")).unwrap();
    std::fs::write(project.path().join("tests/test_math.py"), ORIGINAL_TEST).unwrap();
    std::fs::write(
        project.path().join("math.py"),
        "def add(a, b):\n    return a - b\n",
    )
    .unwrap();
    let telemetry = evidence.path().join("telemetry.jsonl");

    let mut child = Command::new(env!("CARGO_BIN_EXE_peach"))
        .args([
            "exec",
            "fix add",
            "--json",
            "--max-duration-secs",
            "4",
            "--telemetry",
        ])
        .arg(&telemetry)
        .env("PEACH_CONFIG", config.path())
        .env("PEACH_TEST_BOGUS_KEY", "bogus-key-value")
        .current_dir(project.path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    wait_for_run_start(&telemetry);
    std::fs::write(
        project.path().join("tests/test_math.py"),
        "def test_add():\n    pass\n",
    )
    .unwrap();
    std::fs::write(
        project.path().join("tests/test_extra.py"),
        "def test_ok():\n    pass\n",
    )
    .unwrap();

    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if started.elapsed() > BOUND {
            let _ = child.kill();
            panic!("peach exec did not exit within {BOUND:?}");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let mut stdout = String::new();
    child
        .stdout
        .take()
        .unwrap()
        .read_to_string(&mut stdout)
        .unwrap();
    let report: serde_json::Value = stdout
        .lines()
        .rev()
        .find_map(|line| serde_json::from_str(line.trim()).ok())
        .unwrap_or_else(|| panic!("no JSON line in:\n{stdout}"));

    // The time budget is what ended the run; integrity does not change that.
    assert_eq!(status.code(), Some(4), "stdout:\n{stdout}");
    assert_eq!(report["outcome"], "time_budget");

    let actual = &report["integrity"];
    let expected = serde_json::json!({
        "checked": 1,
        "violations": [
            {"path": "tests/test_math.py", "kind": "modified", "restored": true},
            {"path": "tests/test_extra.py", "kind": "added", "restored": true},
        ],
    });
    assert_eq!(actual, &expected);

    // The repository is back to its pre-run state.
    assert_eq!(
        std::fs::read_to_string(project.path().join("tests/test_math.py")).unwrap(),
        ORIGINAL_TEST
    );
    assert!(!project.path().join("tests/test_extra.py").exists());

    // And telemetry says so, in order: start, the check, each restore, end.
    let kinds: Vec<String> = std::fs::read_to_string(&telemetry)
        .unwrap()
        .lines()
        .map(|line| {
            let envelope: serde_json::Value = serde_json::from_str(line).unwrap();
            let event = &envelope["event"];
            match event["kind"].as_str() {
                Some(kind) => format!("{}:{kind}", event["type"].as_str().unwrap()),
                None => event["type"].as_str().unwrap().to_string(),
            }
        })
        .collect();
    // The closed port also produces transport retries, now metered (D-032),
    // the first request records its prompt composition (T6.1), and a run
    // that did not complete says why (HACKATHON section 15, "errors").
    for expected in ["retry", "context_composition", "error:time_budget"] {
        assert!(
            kinds.iter().any(|kind| kind == expected),
            "no {expected}: {kinds:?}"
        );
    }
    let kinds: Vec<&String> = kinds
        .iter()
        .filter(|kind| {
            !matches!(
                kind.as_str(),
                "retry" | "context_composition" | "error:time_budget"
            ) && !kind.starts_with("agent_state")
        })
        .collect();
    assert_eq!(
        kinds,
        vec![
            "run_start",
            "integrity:verify",
            "integrity:restored",
            "integrity:restored",
            "run_end",
        ]
    );
}

#[cfg(unix)]
#[test]
fn test_a_signal_still_restores_tests_and_writes_the_bundle() {
    // A runner's hard timeout or a person stopping the run sends SIGTERM.
    // Before the handler existed, the default action killed peach with no
    // integrity check and no evidence.
    let config = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let evidence = tempfile::tempdir().unwrap();
    write_unroutable_provider_config(config.path());
    std::fs::create_dir_all(project.path().join("tests")).unwrap();
    std::fs::write(project.path().join("tests/test_math.py"), ORIGINAL_TEST).unwrap();
    let bundle = evidence.path().join("bundle");

    let mut child = Command::new(env!("CARGO_BIN_EXE_peach"))
        .args([
            "exec",
            "fix add",
            "--json",
            "--test-command",
            "true",
            "--evidence-dir",
        ])
        .arg(&bundle)
        .env("PEACH_CONFIG", config.path())
        .env("PEACH_TEST_BOGUS_KEY", "bogus-key-value")
        .current_dir(project.path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    wait_for_run_start(&bundle.join("telemetry.jsonl"));
    std::fs::write(
        project.path().join("tests/test_math.py"),
        "def test_add():\n    pass\n",
    )
    .unwrap();
    // Give the handler a moment to be polled once the agent is running.
    std::thread::sleep(Duration::from_millis(300));
    let killed = Command::new("kill")
        .args(["-TERM", &child.id().to_string()])
        .status()
        .unwrap();
    assert!(killed.success());

    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if started.elapsed() > BOUND {
            let _ = child.kill();
            panic!("peach exec did not exit within {BOUND:?} of SIGTERM");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let mut stdout = String::new();
    child
        .stdout
        .take()
        .unwrap()
        .read_to_string(&mut stdout)
        .unwrap();

    assert_eq!(status.code(), Some(5), "stdout:\n{stdout}");
    let exec: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(bundle.join("exec.json")).unwrap()).unwrap();
    assert_eq!(exec["outcome"], "interrupted");
    assert_eq!(exec["error"], "stopped by SIGTERM");
    assert_eq!(exec["integrity"]["violations"][0]["restored"], true);
    assert_eq!(
        std::fs::read_to_string(project.path().join("tests/test_math.py")).unwrap(),
        ORIGINAL_TEST
    );
    let tests: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(bundle.join("tests.json")).unwrap()).unwrap();
    assert_eq!(tests["ran"], false);
    assert!(bundle.join("manifest.json").exists() && bundle.join("report.md").exists());
}

#[cfg(unix)]
#[test]
fn test_a_sigkilled_run_leaves_a_bundle_that_says_so_and_can_be_restored() {
    // SIGKILL cannot be caught, so nothing can run at the end. What a killed
    // run can still leave is what was written at the start: a manifest that
    // says the run never finished, and the integrity baseline with the
    // location of the pre-run copies, so the tests can be checked and restored.
    let config = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let evidence = tempfile::tempdir().unwrap();
    write_unroutable_provider_config(config.path());
    std::fs::create_dir_all(project.path().join("tests")).unwrap();
    std::fs::write(project.path().join("tests/test_math.py"), ORIGINAL_TEST).unwrap();
    let bundle = evidence.path().join("bundle");

    let mut child = Command::new(env!("CARGO_BIN_EXE_peach"))
        .args(["exec", "fix add", "--json", "--evidence-dir"])
        .arg(&bundle)
        .env("PEACH_CONFIG", config.path())
        .env("PEACH_TEST_BOGUS_KEY", "bogus-key-value")
        .current_dir(project.path())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    wait_for_run_start(&bundle.join("telemetry.jsonl"));
    std::fs::write(
        project.path().join("tests/test_math.py"),
        "def test_add():\n    pass\n",
    )
    .unwrap();
    child.kill().unwrap();
    child.wait().unwrap();

    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(bundle.join("manifest.json")).unwrap())
            .unwrap();
    let baseline: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(bundle.join("integrity.baseline.json")).unwrap(),
    )
    .unwrap();
    let snapshot = Path::new(baseline["snapshot_dir"].as_str().unwrap()).join("tests/test_math.py");
    let actual = (
        manifest["outcome"].clone(),
        baseline["files"]["tests/test_math.py"]["bytes"].clone(),
        std::fs::read_to_string(&snapshot).unwrap(),
    );
    let _ = std::fs::remove_dir_all(baseline["snapshot_dir"].as_str().unwrap());

    let expected = (
        serde_json::json!("incomplete"),
        serde_json::json!(ORIGINAL_TEST.len()),
        ORIGINAL_TEST.to_string(),
    );
    assert_eq!(actual, expected);
    assert!(bundle.join("prompt.txt").exists());
}
