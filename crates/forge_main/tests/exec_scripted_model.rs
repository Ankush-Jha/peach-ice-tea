//! harness: R-HACK-1 / R-HACK-2 — `forge exec` driven end to end by a
//! scripted, local, OpenAI-compatible model. No real model is called (₹100
//! total budget, D-025): each scenario is a fixed list of assistant turns.
//!
//! What only a model-driven run can prove, and so what this file is for:
//! - a `followup` call does not end an unattended run mid-task, whether the
//!   agent has the tool (answered in-band) or merely hallucinated its name
//!   (rejected, and the loop carries on);
//! - a `write` to a protected test is refused at dispatch inside the real
//!   binary, with the refusal reaching the model and the telemetry.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use pretty_assertions::assert_eq;

const BOUND: Duration = Duration::from_secs(60);
const ORIGINAL_TEST: &str = "def test_add():\n    assert 1 + 1 == 2\n";

/// One scripted assistant turn.
enum Turn {
    /// Calls one tool with these JSON arguments.
    Tool(&'static str, serde_json::Value),
    /// Replies with text and stops.
    Text(&'static str),
}

/// A local OpenAI-compatible chat endpoint that replays `script` for every
/// request that offers tools (the agent loop), answers anything else (side
/// calls such as title generation) with plain text, and records the bodies of
/// the agent-loop requests.
struct ScriptedModel {
    url: String,
    requests: Arc<Mutex<Vec<String>>>,
}

impl ScriptedModel {
    fn start(script: Vec<Turn>) -> Self {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let url = format!("http://{}/v1/chat/completions", server.server_addr());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = requests.clone();
        std::thread::spawn(move || {
            let mut script = script.into_iter();
            for mut request in server.incoming_requests() {
                let mut body = String::new();
                let _ = request.as_reader().read_to_string(&mut body);
                let turn = if body.contains(r#""tools""#) {
                    recorded.lock().unwrap().push(body);
                    script.next().unwrap_or(Turn::Text("Done."))
                } else {
                    Turn::Text("Scripted title")
                };
                let response = tiny_http::Response::from_string(sse(turn)).with_header(
                    "Content-Type: text/event-stream".parse::<tiny_http::Header>().unwrap(),
                );
                let _ = request.respond(response);
            }
        });
        Self { url, requests }
    }

    fn requests(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }
}

fn sse(turn: Turn) -> String {
    let (delta, finish) = match turn {
        Turn::Tool(name, arguments) => (
            serde_json::json!({"role": "assistant", "tool_calls": [{
                "index": 0, "id": "call_1", "type": "function",
                "function": {"name": name, "arguments": arguments.to_string()},
            }]}),
            "tool_calls",
        ),
        Turn::Text(text) => (serde_json::json!({"role": "assistant", "content": text}), "stop"),
    };
    let chunk = |delta: serde_json::Value, finish: Option<&str>| {
        serde_json::json!({
            "id": "chatcmpl-scripted", "object": "chat.completion.chunk", "created": 0,
            "model": "scripted-model",
            "choices": [{"index": 0, "delta": delta, "finish_reason": finish}],
            "usage": finish.map(|_| serde_json::json!(
                {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15})),
        })
    };
    format!(
        "data: {}\n\ndata: {}\n\ndata: [DONE]\n\n",
        chunk(delta, None),
        chunk(serde_json::json!({}), Some(finish))
    )
}

struct Run {
    exit_code: Option<i32>,
    report: serde_json::Value,
    telemetry: Vec<serde_json::Value>,
}

/// Runs `forge exec` in `project` against `model`, with an isolated
/// `FORGE_CONFIG`. `agent_md` installs a custom agent and selects it.
fn run_exec(project: &Path, model: &ScriptedModel, agent_md: Option<(&str, &str)>) -> Run {
    let config = tempfile::tempdir().unwrap();
    let telemetry = config.path().join("telemetry.jsonl");
    std::fs::write(
        config.path().join(".forge.toml"),
        format!(
            r#"
[[providers]]
id = "scripted"
url = "{}"
response_type = "OpenAI"
auth_methods = ["api_key"]
api_key_var = "FORGE_TEST_SCRIPTED_KEY"

[[providers.models]]
id = "scripted-model"
name = "Scripted"
tools_supported = true
input_modalities = ["text"]

[session]
provider_id = "scripted"
model_id = "scripted-model"
"#,
            model.url
        ),
    )
    .unwrap();

    let mut command = Command::new(env!("CARGO_BIN_EXE_forge"));
    if let Some((id, markdown)) = agent_md {
        std::fs::create_dir_all(config.path().join("agents")).unwrap();
        std::fs::write(config.path().join("agents").join(format!("{id}.md")), markdown).unwrap();
        command.args(["--agent", id]);
    }
    let mut child = command
        .args(["exec", "fix add", "--json", "--max-duration-secs", "45", "--telemetry"])
        .arg(&telemetry)
        .env("FORGE_CONFIG", config.path())
        .env("FORGE_TEST_SCRIPTED_KEY", "scripted-key")
        .current_dir(project)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut stdout_pipe = child.stdout.take().unwrap();
    let reader = std::thread::spawn(move || {
        let mut out = String::new();
        let _ = stdout_pipe.read_to_string(&mut out);
        out
    });

    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if started.elapsed() > BOUND {
            let _ = child.kill();
            panic!("forge exec did not exit within {BOUND:?}");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let stdout = reader.join().unwrap();
    let report = stdout
        .lines()
        .rev()
        .find_map(|line| serde_json::from_str(line.trim()).ok())
        .unwrap_or_else(|| panic!("no JSON line in:\n{stdout}"));
    let telemetry = std::fs::read_to_string(&telemetry)
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap()["event"].clone())
        .collect();
    Run { exit_code: status.code(), report, telemetry }
}

fn project_with_a_test() -> tempfile::TempDir {
    let project = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(project.path().join("tests")).unwrap();
    std::fs::write(project.path().join("tests/test_math.py"), ORIGINAL_TEST).unwrap();
    std::fs::write(project.path().join("math.py"), "def add(a, b):\n    return a - b\n").unwrap();
    project
}

#[test]
fn test_a_hallucinated_followup_does_not_end_the_run() {
    // The default `forge` agent has no `followup` tool. Before the fix the
    // orchestrator yielded on the name alone, so this run ended after one
    // model call and reported "completed" without ever finishing.
    let project = project_with_a_test();
    let model = ScriptedModel::start(vec![
        Turn::Tool("followup", serde_json::json!({"question": "Which file?"})),
        Turn::Text("Done."),
    ]);

    let run = run_exec(project.path(), &model, None);

    assert_eq!(run.exit_code, Some(0), "report: {}", run.report);
    assert_eq!(run.report["outcome"], "completed");
    assert_eq!(model.requests().len(), 2, "the loop must continue after the followup call");
}

#[test]
fn test_an_agent_with_followup_is_answered_instead_of_prompting() {
    let project = project_with_a_test();
    let model = ScriptedModel::start(vec![
        Turn::Tool("followup", serde_json::json!({"question": "Which file?"})),
        Turn::Text("Done."),
    ]);
    let agent = "---\nid: \"asker\"\ntitle: \"Asks questions\"\ndescription: \"Test agent that can ask\"\ntools:\n  - followup\n  - read\n---\nYou are a test agent.\n";

    let run = run_exec(project.path(), &model, Some(("asker", agent)));

    assert_eq!(run.exit_code, Some(0), "report: {}", run.report);
    let requests = model.requests();
    assert_eq!(requests.len(), 2, "the loop must continue after the followup call");
    assert!(
        requests[1].contains("No human is available to answer"),
        "the model must receive the unattended answer as the tool result"
    );
    assert!(
        run.telemetry
            .iter()
            .any(|event| event["type"] == "prompt_suppressed" && event["prompt_kind"] == "followup"),
        "telemetry: {:?}",
        run.telemetry
    );
}

#[test]
fn test_a_write_to_a_protected_test_is_refused_at_dispatch() {
    let project = project_with_a_test();
    let model = ScriptedModel::start(vec![
        Turn::Tool(
            "write",
            serde_json::json!({
                "file_path": project.path().join("tests/test_math.py"),
                "content": "def test_add():\n    pass\n",
                "overwrite": true,
            }),
        ),
        Turn::Text("Done."),
    ]);

    let run = run_exec(project.path(), &model, None);

    assert_eq!(run.exit_code, Some(0), "report: {}", run.report);
    // Refused before it ran: the file never changed, so the post-run check
    // is clean rather than "restored".
    assert_eq!(
        std::fs::read_to_string(project.path().join("tests/test_math.py")).unwrap(),
        ORIGINAL_TEST
    );
    assert_eq!(run.report["integrity"], serde_json::json!({"checked": 1, "violations": []}));
    let requests = model.requests();
    assert!(
        requests[1].contains("because it is a protected test file"),
        "the model must see the refusal"
    );
    // The protected-file notice reached the model before it acted.
    assert!(requests[0].contains("PROTECTED TEST FILES"));
    let refusals: Vec<&serde_json::Value> = run
        .telemetry
        .iter()
        .filter(|event| event["type"] == "integrity" && event["kind"] == "refused")
        .collect();
    assert_eq!(refusals.len(), 1, "telemetry: {:?}", run.telemetry);
    assert_eq!(refusals[0]["path"], "tests/test_math.py");
}
