//! harness: R-HACK-1 / R-HACK-2 — `peach exec` driven end to end by a
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

use bstr::ByteSlice;
use pretty_assertions::assert_eq;

const BOUND: Duration = Duration::from_secs(60);
const ORIGINAL_TEST: &str = "def test_add():\n    assert 1 + 1 == 2\n";

/// One scripted assistant turn.
enum Turn {
    /// Calls one tool with these JSON arguments.
    Tool(&'static str, serde_json::Value),
    /// Replies with text and stops.
    Text(&'static str),
    /// Fails with this HTTP status, as a rate limit or outage would.
    Status(u16),
    /// Fails with this HTTP status and JSON body.
    StatusBody(u16, &'static str),
    /// Calls several tools in one turn.
    Tools(Vec<(&'static str, serde_json::Value)>),
    /// An empty completion: no content, no tool call, no finish reason —
    /// with this `(prompt, completion)` usage, or none at all.
    Empty(Option<(u64, u64)>),
    /// A DeepSeek thinking-mode turn: streamed `reasoning_content`, then one
    /// tool call (or text, when the name is empty), with DeepSeek-shaped
    /// usage (`prompt_cache_hit_tokens`, reasoning in
    /// `completion_tokens_details`).
    DeepSeek { reasoning: &'static str, tool: &'static str, arguments: serde_json::Value, text: &'static str },
}

/// A local OpenAI-compatible chat endpoint that replays `script` for every
/// request that offers tools (the agent loop), answers anything else (side
/// calls such as title generation) with plain text, and records the bodies of
/// the agent-loop requests.
struct ScriptedModel {
    url: String,
    requests: Arc<Mutex<Vec<String>>>,
    /// Requests that offered no tools (side calls such as title generation).
    side_requests: Arc<Mutex<usize>>,
    /// Provider id the mock is registered as; `deepseek` engages peach's
    /// DeepSeek-specific request transformers.
    provider_id: &'static str,
}

impl ScriptedModel {
    fn start(script: Vec<Turn>) -> Self {
        Self::start_as("scripted", script)
    }

    fn start_as(provider_id: &'static str, script: Vec<Turn>) -> Self {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let url = format!("http://{}/v1/chat/completions", server.server_addr());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = requests.clone();
        let side_requests = Arc::new(Mutex::new(0usize));
        let side_recorded = side_requests.clone();
        std::thread::spawn(move || {
            let mut script = script.into_iter();
            for mut request in server.incoming_requests() {
                let mut body = String::new();
                let _ = request.as_reader().read_to_string(&mut body);
                let turn = if body.contains(r#""tools""#) {
                    recorded.lock().unwrap().push(body);
                    script.next().unwrap_or(Turn::Text("Done."))
                } else {
                    *side_recorded.lock().unwrap() += 1;
                    Turn::Text("Scripted title")
                };
                let response = match turn {
                    Turn::Status(code) => tiny_http::Response::from_string("{}")
                        .with_status_code(code),
                    Turn::StatusBody(code, body) => tiny_http::Response::from_string(body)
                        .with_status_code(code),
                    turn => tiny_http::Response::from_string(sse(turn)).with_header(
                        "Content-Type: text/event-stream".parse::<tiny_http::Header>().unwrap(),
                    ),
                };
                let _ = request.respond(response);
            }
        });
        Self { url, requests, side_requests, provider_id }
    }

    fn requests(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }
}

fn sse(turn: Turn) -> String {
    if let Turn::DeepSeek { reasoning, tool, arguments, text } = turn {
        let chunk = |delta: serde_json::Value, finish: Option<&str>, usage: Option<serde_json::Value>| {
            serde_json::json!({
                "id": "chatcmpl-ds", "object": "chat.completion.chunk", "created": 0,
                "model": "deepseek-v4-flash",
                "choices": [{"index": 0, "delta": delta, "finish_reason": finish}],
                "usage": usage,
            })
        };
        let (answer, finish) = if tool.is_empty() {
            (serde_json::json!({"content": text}), "stop")
        } else {
            (
                serde_json::json!({"tool_calls": [{"index": 0, "id": "call_ds", "type": "function",
                    "function": {"name": tool, "arguments": arguments.to_string()}}]}),
                "tool_calls",
            )
        };
        let usage = serde_json::json!({
            "prompt_tokens": 1000, "completion_tokens": 40, "total_tokens": 1040,
            "prompt_cache_hit_tokens": 600, "prompt_cache_miss_tokens": 400,
            "prompt_tokens_details": {"prompt_cache_hit_tokens": 600, "prompt_cache_miss_tokens": 400},
            "completion_tokens_details": {"reasoning_tokens": 25},
        });
        return [
            chunk(serde_json::json!({"role": "assistant", "reasoning_content": reasoning}), None, None),
            chunk(answer, None, None),
            chunk(serde_json::json!({}), Some(finish), Some(usage)),
        ]
        .iter()
        .map(|chunk| format!("data: {chunk}\n\n"))
        .collect::<String>()
            + "data: [DONE]\n\n";
    }
    if let Turn::Empty(usage) = turn {
        let usage = usage.map(|(prompt, completion)| {
            serde_json::json!({"prompt_tokens": prompt, "completion_tokens": completion,
                "total_tokens": prompt + completion})
        });
        let chunk = serde_json::json!({
            "id": "chatcmpl-scripted", "object": "chat.completion.chunk", "created": 0,
            "model": "scripted-model",
            "choices": [{"index": 0, "delta": {}, "finish_reason": null}],
            "usage": usage,
        });
        return format!("data: {chunk}\n\ndata: [DONE]\n\n");
    }
    let (delta, finish) = match turn {
        Turn::Tool(name, arguments) => (
            serde_json::json!({"role": "assistant", "tool_calls": [{
                "index": 0, "id": "call_1", "type": "function",
                "function": {"name": name, "arguments": arguments.to_string()},
            }]}),
            "tool_calls",
        ),
        Turn::Tools(calls) => (
            serde_json::json!({"role": "assistant", "tool_calls": calls.iter().enumerate().map(|(index, (name, arguments))| {
                serde_json::json!({"index": index, "id": format!("call_{index}"), "type": "function",
                    "function": {"name": name, "arguments": arguments.to_string()}})
            }).collect::<Vec<_>>()}),
            "tool_calls",
        ),
        Turn::Text(text) => (serde_json::json!({"role": "assistant", "content": text}), "stop"),
        Turn::Status(_) | Turn::StatusBody(..) | Turn::Empty(_) | Turn::DeepSeek { .. } => {
            unreachable!("handled above")
        }
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

/// Runs `peach exec` in `project` against `model`, with an isolated
/// `PEACH_CONFIG`. `agent_md` installs a custom agent and selects it.
fn run_exec(project: &Path, model: &ScriptedModel, agent_md: Option<(&str, &str)>) -> Run {
    run_exec_with_env(project, model, agent_md, &[])
}

/// [`run_exec`] with extra environment variables for the peach process.
fn run_exec_with_env(
    project: &Path,
    model: &ScriptedModel,
    agent_md: Option<(&str, &str)>,
    extra_env: &[(&str, &str)],
) -> Run {
    run_exec_full(project, model, agent_md, extra_env, "fix add", &[])
}

/// Runs `peach exec <task>` with extra environment and arguments.
fn run_exec_full(
    project: &Path,
    model: &ScriptedModel,
    agent_md: Option<(&str, &str)>,
    extra_env: &[(&str, &str)],
    task: &str,
    extra_args: &[&str],
) -> Run {
    run_exec_configured(project, model, agent_md, extra_env, task, extra_args, "")
}

/// [`run_exec_full`] with `extra_toml` appended to the generated config.
fn run_exec_configured(
    project: &Path,
    model: &ScriptedModel,
    agent_md: Option<(&str, &str)>,
    extra_env: &[(&str, &str)],
    task: &str,
    extra_args: &[&str],
    extra_toml: &str,
) -> Run {
    let config = tempfile::tempdir().unwrap();
    let telemetry = config.path().join("telemetry.jsonl");
    std::fs::write(
        config.path().join(".peach.toml"),
        format!(
            r#"
[[providers]]
id = "{provider}"
url = "{url}"
response_type = "OpenAI"
auth_methods = ["api_key"]
api_key_var = "PEACH_TEST_SCRIPTED_KEY"

[[providers.models]]
id = "scripted-model"
name = "Scripted"
tools_supported = true
input_modalities = ["text"]

[session]
provider_id = "{provider}"
model_id = "scripted-model"
{extra_toml}
"#,
            provider = model.provider_id,
            url = model.url,
        ),
    )
    .unwrap();

    let mut command = Command::new(env!("CARGO_BIN_EXE_peach"));
    if let Some((id, markdown)) = agent_md {
        std::fs::create_dir_all(config.path().join("agents")).unwrap();
        std::fs::write(config.path().join("agents").join(format!("{id}.md")), markdown).unwrap();
        command.args(["--agent", id]);
    }
    command.envs(extra_env.iter().copied());
    let mut child = command
        .args(["exec", task, "--json", "--max-duration-secs", "45", "--telemetry"])
        .arg(&telemetry)
        .args(extra_args)
        .env("PEACH_CONFIG", config.path())
        .env("PEACH_TEST_SCRIPTED_KEY", "scripted-key")
        // Real retry behaviour, shorter waits, so retry scenarios stay fast.
        .env("PEACH_RETRY__MIN_DELAY_MS", "10")
        .env("PEACH_RETRY__INITIAL_BACKOFF_MS", "10")
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
            panic!("peach exec did not exit within {BOUND:?}");
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
    // The default `peach` agent has no `followup` tool. Before the fix the
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

#[test]
fn test_retries_are_metered_and_the_telemetry_stream_is_complete() {
    // D-032's live run: 17 retries (429/503/empty completions) that neither
    // metrics nor telemetry showed. This replays that shape locally.
    let project = project_with_a_test();
    let model = ScriptedModel::start(vec![
        Turn::Status(503),
        Turn::Empty(None),
        Turn::Empty(Some((14_000, 900))),
        Turn::Tool("shell", serde_json::json!({"command": "echo hi", "description": "say hi"})),
        Turn::Text("Done."),
    ]);

    let run = run_exec(project.path(), &model, None);

    assert_eq!(run.exit_code, Some(0), "report: {}", run.report);
    let metrics = &run.report["metrics"];
    // Semantic calls: two requests, however many attempts the first took.
    assert_eq!(metrics["llm_calls"], 2);
    assert_eq!(metrics["retried_llm_calls"], 3);
    // The empty completion that reported usage is billed into the totals:
    // 14,000 + 10 + 10 input, 900 + 5 + 5 output.
    assert_eq!(metrics["input_tokens"], 14_020);
    assert_eq!(metrics["output_tokens"], 910);

    let kinds: Vec<String> = run
        .telemetry
        .iter()
        .map(|event| match event["type"].as_str().unwrap() {
            "agent_state" => format!("agent_state:{}", event["to"].as_str().unwrap()),
            "integrity" => format!("integrity:{}", event["kind"].as_str().unwrap()),
            "tool_call" => format!("tool_call:{}", event["name"].as_str().unwrap()),
            other => other.to_string(),
        })
        .collect();
    assert_eq!(
        kinds,
        vec![
            "run_start",
            "agent_state:running",
            "context_composition",
            "retry",
            "retry",
            "retry",
            "model_call",
            "tool_call:shell",
            "model_call",
            "agent_state:ended",
            "integrity:verify",
            "run_end",
        ]
    );

    // T6.1: the first request's fixed cost, by source. D-039 measured tool
    // definitions as its largest part.
    let composition = run.telemetry.iter().find(|event| event["type"] == "context_composition").unwrap();
    let sources = &composition["tokens_by_source_estimated"];
    let tools = sources["tool_definitions"].as_u64().unwrap();
    assert!(tools > sources["system_prompt"].as_u64().unwrap(), "{sources}");
    assert!(tools > sources["user_prompt"].as_u64().unwrap(), "{sources}");

    let retries: Vec<&serde_json::Value> =
        run.telemetry.iter().filter(|event| event["type"] == "retry").collect();
    // 503: nothing to bill, so no usage claim either way.
    assert!(retries[0].get("usage_reported").is_none(), "{}", retries[0]);
    assert!(retries[0]["reason"].as_str().unwrap().contains("503"));
    // Empty completion with no usage: explicitly unknown, not zero.
    assert_eq!(retries[1]["usage_reported"], false);
    assert!(retries[1].get("input_tokens").is_none());
    // Empty completion with usage: its tokens are on the event.
    assert_eq!(retries[2]["usage_reported"], true);
    assert_eq!(retries[2]["input_tokens"], 14_000);
    assert_eq!(retries[2]["output_tokens"], 900);

    let model_calls: Vec<&serde_json::Value> =
        run.telemetry.iter().filter(|event| event["type"] == "model_call").collect();
    assert_eq!(model_calls[0]["input_tokens"], 10);
    assert_eq!(model_calls[0]["finish_reason"], "tool_calls");
    let tool = run.telemetry.iter().find(|event| event["type"] == "tool_call").unwrap();
    assert_eq!(tool["success"], true);
    assert_eq!(tool["origin_call_id"], model_calls[0]["call_id"]);
    assert_eq!(model_calls[0]["tool_call_ids"], serde_json::json!([tool["call_id"]]));
}

#[test]
fn test_a_compaction_is_reported_in_telemetry() {
    let project = project_with_a_test();
    let echo = |text: &'static str| {
        Turn::Tool("shell", serde_json::json!({"command": format!("echo {text}"), "description": "echo"}))
    };
    let model = ScriptedModel::start(vec![
        echo("one"),
        echo("two"),
        echo("three"),
        echo("four"),
        Turn::Text("Done."),
    ]);

    let run = run_exec_with_env(
        project.path(),
        &model,
        None,
        &[("PEACH_COMPACT__MESSAGE_THRESHOLD", "6"), ("PEACH_COMPACT__RETENTION_WINDOW", "2")],
    );

    assert_eq!(run.exit_code, Some(0), "report: {}", run.report);
    let compactions: Vec<&serde_json::Value> = run
        .telemetry
        .iter()
        .filter(|event| event["type"] == "context_compaction")
        .collect();
    assert!(!compactions.is_empty(), "telemetry: {:?}", run.telemetry);
    assert_eq!(
        compactions.len() as u64,
        run.report["metrics"]["compactions"]["count"].as_u64().unwrap(),
        "one event per compaction the metrics counted"
    );
    let first = compactions[0];
    assert!(first["messages_after"].as_u64() < first["messages_before"].as_u64(), "{first}");
}

fn git(repo: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.email=t@t", "-c", "user.name=t"])
        .args(args)
        .current_dir(repo)
        .stdout(Stdio::null())
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?}");
}

fn git_project_with_a_test() -> tempfile::TempDir {
    let project = project_with_a_test();
    git(project.path(), &["init", "-q"]);
    git(project.path(), &["add", "-A"]);
    git(project.path(), &["commit", "-qm", "start"]);
    project
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Every file in `dir`, recursively, as (relative path, bytes).
fn bundle_files(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut files = vec![];
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_file() {
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            files.push((name, std::fs::read(&path).unwrap()));
        }
    }
    files.sort();
    files
}

#[test]
fn test_a_completed_run_writes_a_complete_redacted_evidence_bundle() {
    let project = git_project_with_a_test();
    let evidence = tempfile::tempdir().unwrap();
    let key = format!("AIza{}", "q".repeat(35));
    let task = format!("fix add; the staging key {key} is irrelevant");
    let model = ScriptedModel::start(vec![
        // Peach refuses to overwrite a file the model has not read.
        Turn::Tool("read", serde_json::json!({"file_path": project.path().join("math.py")})),
        Turn::Tool(
            "write",
            serde_json::json!({
                "file_path": project.path().join("math.py"),
                "content": "def add(a, b):\n    return a + b\n",
                "overwrite": true,
            }),
        ),
        Turn::Text("Done."),
    ]);
    let dir = evidence.path().join("bundle");

    let run = run_exec_full(
        project.path(),
        &model,
        None,
        &[],
        &task,
        &["--evidence-dir", dir.to_str().unwrap()],
    );

    assert_eq!(run.exit_code, Some(0), "report: {}", run.report);
    let files = bundle_files(&dir);
    let names: Vec<&str> = files.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(
        names,
        vec![
            "diff.patch",
            "exec.json",
            "integrity.json",
            "manifest.json",
            "prompt.txt",
            "report.json",
            "report.md",
            "tests.json",
            "transcript.json",
        ],
        "telemetry went to --telemetry, which overrides the bundle default"
    );
    let read = |name: &str| String::from_utf8(files.iter().find(|(n, _)| n == name).unwrap().1.clone()).unwrap();

    // The frozen prompt is the task as given, not with the harness notice.
    assert_eq!(read("prompt.txt"), task.replace(&key, "[REDACTED]"));
    assert!(
        read("diff.patch").contains("+    return a + b"),
        "diff: {:?}\nmath.py: {:?}\ntelemetry: {:?}",
        read("diff.patch"),
        std::fs::read_to_string(project.path().join("math.py")),
        run.telemetry.iter().filter(|e| e["type"] == "tool_call").collect::<Vec<_>>()
    );
    let exec: serde_json::Value = serde_json::from_str(&read("exec.json")).unwrap();
    assert_eq!(exec["outcome"], "completed");
    // The harness found the project's tests itself and ran them.
    let tests: serde_json::Value = serde_json::from_str(&read("tests.json")).unwrap();
    assert_eq!((tests["ran"].as_bool(), tests["source"].as_str()), (Some(true), Some("tests/test_*.py")));
    let transcript: serde_json::Value = serde_json::from_str(&read("transcript.json")).unwrap();
    assert_eq!(transcript["conversation"]["id"], run.report["conversation_id"]);

    // The manifest checksums exactly the other files, as they are on disk.
    let manifest: serde_json::Value = serde_json::from_str(&read("manifest.json")).unwrap();
    let expected: serde_json::Map<String, serde_json::Value> = files
        .iter()
        .filter(|(name, _)| name != "manifest.json")
        .map(|(name, bytes)| (name.clone(), serde_json::Value::String(sha256_hex(bytes))))
        .collect();
    assert_eq!(manifest["files"], serde_json::Value::Object(expected));

    // The report agrees with the exec outcome it was built from.
    let report: serde_json::Value = serde_json::from_str(&read("report.json")).unwrap();
    assert_eq!(report["outcome"]["outcome"], "completed");
    assert_eq!(report["tokens"]["input"], exec["metrics"]["input_tokens"]);
    assert_eq!(report["model_calls"]["calls"], exec["metrics"]["llm_calls"]);
    assert_eq!(report["repository"]["files"][0]["path"], "math.py");

    // `peach report` rebuilds the same report from the bundle alone: no
    // provider, no model, nothing re-run.
    let empty_config = tempfile::tempdir().unwrap();
    std::fs::remove_file(dir.join("report.md")).unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_peach"))
        .args(["report", dir.to_str().unwrap()])
        .env("PEACH_CONFIG", empty_config.path())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .status()
        .unwrap();
    assert!(status.success());
    assert_eq!(std::fs::read_to_string(dir.join("report.md")).unwrap(), read("report.md"));

    // The key reached the model (it was in the task) but not the bundle.
    for (name, bytes) in &files {
        assert!(!bytes.to_str_lossy().contains(&key), "{name} leaks the key");
    }
}

#[test]
fn test_an_errored_run_still_writes_its_bundle() {
    let project = git_project_with_a_test();
    let evidence = tempfile::tempdir().unwrap();
    // 400 is not retryable: the run fails on its first request.
    let model = ScriptedModel::start(vec![Turn::Status(400)]);
    let dir = evidence.path().join("bundle");

    let run = run_exec_full(
        project.path(),
        &model,
        None,
        &[],
        "fix add",
        &["--evidence-dir", dir.to_str().unwrap()],
    );

    assert_eq!(run.exit_code, Some(1), "report: {}", run.report);
    let exec: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("exec.json")).unwrap()).unwrap();
    assert_eq!(exec["outcome"], "error");
    for name in ["prompt.txt", "integrity.json", "diff.patch", "tests.json", "transcript.json", "manifest.json"] {
        assert!(dir.join(name).exists(), "{name} missing on the error path");
    }
}

#[test]
fn test_deepseek_thinking_mode_runs_with_full_accounting() {
    // Registered as provider `deepseek`, so peach's DeepSeek request
    // transformers run for real: reasoning is replayed as a flat
    // `reasoning_content` on the assistant message.
    let project = git_project_with_a_test();
    let evidence = tempfile::tempdir().unwrap();
    let model = ScriptedModel::start_as(
        "deepseek",
        vec![
            Turn::DeepSeek {
                reasoning: "The subtraction is the bug; read the file first.",
                tool: "read",
                arguments: serde_json::json!({"file_path": project.path().join("math.py")}),
                text: "",
            },
            Turn::DeepSeek { reasoning: "Done reading.", tool: "", arguments: serde_json::json!({}), text: "Done." },
        ],
    );
    let dir = evidence.path().join("bundle");

    let run = run_exec_full(
        project.path(),
        &model,
        None,
        &[],
        "fix add",
        &["--evidence-dir", dir.to_str().unwrap()],
    );

    assert_eq!(run.exit_code, Some(0), "report: {}", run.report);
    let metrics = &run.report["metrics"];
    assert_eq!(metrics["llm_calls"], 2);
    assert_eq!(metrics["input_tokens"], 2000);
    // DeepSeek's cache hits, which peach used to read as zero.
    assert_eq!(metrics["cached_input_tokens"], 1200);
    assert_eq!(metrics["reasoning_tokens"], 50);

    // The second request replays the first turn's reasoning the DeepSeek way.
    let requests = model.requests();
    let second: serde_json::Value = serde_json::from_str(&requests[1]).unwrap();
    let assistant = second["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|message| message["role"] == "assistant")
        .expect("assistant turn replayed");
    assert_eq!(assistant["reasoning_content"], "The subtraction is the bug; read the file first.");
    // The harness's protected-file notice reached DeepSeek too.
    assert!(requests[0].contains("PROTECTED TEST FILES"));

    let model_call = run.telemetry.iter().find(|event| event["type"] == "model_call").unwrap();
    assert_eq!(model_call["cached_tokens"], 600);
    let report: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("report.json")).unwrap()).unwrap();
    assert_eq!(report["tokens"]["cache_hit_rate"], 0.6);
    assert_eq!(report["integrity"]["violations"], serde_json::json!([]));
}

/// A git project whose suite runs under the standard library's unittest:
/// `calc.add` subtracts, and `tests/test_calc.py` expects addition.
fn calc_project() -> tempfile::TempDir {
    let project = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(project.path().join("tests")).unwrap();
    std::fs::write(project.path().join("calc.py"), "def add(a, b):\n    return a - b\n").unwrap();
    std::fs::write(project.path().join("tests/__init__.py"), "").unwrap();
    std::fs::write(
        project.path().join("tests/test_calc.py"),
        "import unittest\n\nfrom calc import add\n\n\nclass TestAdd(unittest.TestCase):\n    def test_add(self):\n        self.assertEqual(add(1, 2), 3)\n",
    )
    .unwrap();
    git(project.path(), &["init", "-q"]);
    git(project.path(), &["add", "-A"]);
    git(project.path(), &["commit", "-qm", "start"]);
    project
}

const CALC_TESTS: &str = "python3 -m unittest discover -s tests -t . -v";

fn fix_calc(project: &Path) -> Vec<Turn> {
    vec![
        Turn::Tool("read", serde_json::json!({"file_path": project.join("calc.py")})),
        Turn::Tool(
            "write",
            serde_json::json!({
                "file_path": project.join("calc.py"),
                "content": "def add(a, b):\n    return a + b\n",
                "overwrite": true,
            }),
        ),
    ]
}

#[test]
fn test_an_unverified_finish_is_sent_back_to_run_the_tests() {
    let project = calc_project();
    let evidence = tempfile::tempdir().unwrap();
    let mut script = fix_calc(project.path());
    script.push(Turn::Text("Fixed."));
    script.push(Turn::Tool("shell", serde_json::json!({"command": CALC_TESTS, "description": "run tests"})));
    script.push(Turn::Text("Fixed and verified."));
    let model = ScriptedModel::start(script);
    let dir = evidence.path().join("bundle");

    let run = run_exec_full(
        project.path(),
        &model,
        None,
        &[],
        "fix add",
        &["--evidence-dir", dir.to_str().unwrap(), "--test-command", CALC_TESTS],
    );

    assert_eq!(run.exit_code, Some(0), "report: {}", run.report);
    let requests = model.requests();
    assert_eq!(requests.len(), 5, "read, write, finish (sent back), test run, finish");
    assert!(requests[3].contains("VERIFICATION REQUIRED"));
    assert!(requests[3].contains(CALC_TESTS));
    assert!(
        !requests[4].contains("before finishing (2 of 2)"),
        "no second reminder after a green run"
    );

    let test_runs: Vec<(String, String)> = run
        .telemetry
        .iter()
        .filter(|event| event["type"] == "test_run")
        .map(|event| {
            (event["origin"].as_str().unwrap().to_string(), event["failure_class"].as_str().unwrap().to_string())
        })
        .collect();
    assert_eq!(
        test_runs,
        vec![("agent".to_string(), "passed".to_string()), ("harness_final".to_string(), "passed".to_string())]
    );
    let tests: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("tests.json")).unwrap()).unwrap();
    assert_eq!(tests["class"], "passed");
    assert_eq!(tests["passed"], 1);
    assert_eq!(tests["source"], "explicit");
}

#[test]
fn test_the_gate_gives_up_after_two_reminders_and_says_so() {
    let project = calc_project();
    let evidence = tempfile::tempdir().unwrap();
    let mut script = fix_calc(project.path());
    script.extend([Turn::Text("Fixed."), Turn::Text("Still fixed."), Turn::Text("Really fixed.")]);
    let model = ScriptedModel::start(script);
    let dir = evidence.path().join("bundle");

    // No --test-command: detection must find the unittest suite on its own.
    let run = run_exec_full(project.path(), &model, None, &[], "fix add", &["--evidence-dir", dir.to_str().unwrap()]);

    assert_eq!(run.exit_code, Some(0), "report: {}", run.report);
    let requests = model.requests();
    assert_eq!(requests.len(), 5, "two reminders, then the run is allowed to end");
    assert!(requests[3].contains("before finishing (1 of 2)"));
    assert!(!requests[3].contains("before finishing (2 of 2)"));
    assert!(requests[4].contains("before finishing (2 of 2)"));
    assert!(
        run.telemetry.iter().any(|event| event["type"] == "agent_state" && event["to"] == "verification_unconfirmed"),
        "telemetry: {:?}",
        run.telemetry
    );
    // The harness still checks: the fix was right, so its own run passes.
    let tests: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("tests.json")).unwrap()).unwrap();
    assert_eq!((tests["class"].as_str(), tests["source"].as_str()), (Some("passed"), Some("tests/test_*.py")));
}

#[test]
fn test_a_test_run_that_never_reached_the_code_gets_a_recovery_hint() {
    let project = calc_project();
    let model = ScriptedModel::start(vec![
        Turn::Tool("read", serde_json::json!({"file_path": project.path().join("calc.py")})),
        Turn::Tool(
            "write",
            serde_json::json!({
                "file_path": project.path().join("calc.py"),
                "content": "def add(a, b)\n    return a + b\n",
                "overwrite": true,
            }),
        ),
        Turn::Tool("shell", serde_json::json!({"command": CALC_TESTS, "description": "run tests"})),
        Turn::Text("Done."),
    ]);

    let run = run_exec_full(project.path(), &model, None, &[], "fix add", &["--test-command", CALC_TESTS]);

    assert_eq!(run.exit_code, Some(0), "report: {}", run.report);
    let requests = model.requests();
    assert!(requests[3].contains("RECOVERY HINT (harness): the code did not compile"), "no hint after the broken run");
    assert!(!requests[2].contains("RECOVERY HINT"), "hints only follow a failed test run");
    let test_run = run.telemetry.iter().find(|event| event["type"] == "test_run").unwrap();
    assert_eq!(test_run["failure_class"], "compile");
    assert!(
        run.telemetry
            .iter()
            .any(|event| event["type"] == "recovery" && event["trigger"] == "compile"),
        "telemetry: {:?}",
        run.telemetry
    );
}

#[test]
fn test_line_numbers_flag_changes_only_the_default() {
    let project = calc_project();
    let file = project.path().join("calc.py");
    let model = ScriptedModel::start(vec![
        Turn::Tool("read", serde_json::json!({"file_path": file})),
        Turn::Tool("read", serde_json::json!({"file_path": file, "show_line_numbers": true})),
        Turn::Text("Done."),
    ]);

    let run = run_exec_with_env(project.path(), &model, None, &[("PEACH_HARNESS_LINE_NUMBERS_OFF", "1")]);

    assert_eq!(run.exit_code, Some(0), "report: {}", run.report);
    let requests = model.requests();
    // The tool results are JSON-escaped inside the request body.
    assert!(requests[1].contains("def add(a, b):\\n    return a - b"), "unnumbered read expected");
    assert!(!requests[1].contains("1:def add"));
    assert!(requests[2].contains("1:def add"), "an explicit request for numbers is honoured");
}

#[test]
fn test_compacted_tool_docs_shrink_every_request_and_drop_only_examples() {
    let run_first_request = |flag: &str| {
        let project = calc_project();
        let model = ScriptedModel::start(vec![Turn::Text("Done.")]);
        let run = run_exec_with_env(project.path(), &model, None, &[("PEACH_HARNESS_COMPACT_TOOL_DOCS", flag)]);
        assert_eq!(run.exit_code, Some(0), "report: {}", run.report);
        model.requests()[0].clone()
    };

    let full = run_first_request("0");
    let compact = run_first_request("1");

    assert!(full.contains("<example"));
    let body: serde_json::Value = serde_json::from_str(&compact).unwrap();
    let tools = body["tools"].to_string();
    let where_found: Vec<String> = compact
        .match_indices("<example")
        .map(|(i, _)| compact[i.saturating_sub(80)..(i + 40).min(compact.len())].to_string())
        .collect();
    // Tools only: the system prompt has examples of its own, which this flag
    // deliberately leaves alone (compressing it is T6.2, an A/B task).
    assert!(!tools.contains("<example"), "in tools: {where_found:?}");
    // Rules survive: a line from todo_write's rules and one from task's notes.
    assert!(compact.contains("Exactly ONE task must be"));
    assert!(compact.contains("Launch multiple agents concurrently whenever possible"));
    assert!(
        compact.len() * 100 < full.len() * 90,
        "expected at least a 10% smaller request: {} -> {} bytes",
        full.len(),
        compact.len()
    );
}

#[test]
fn test_an_exhausted_daily_quota_fails_at_once_and_says_why() {
    // D-040: exactly what Gemini's free tier returned on 2026-09-25. Before,
    // peach retried it 8 times over 5.5 minutes and the evidence said only
    // "Invalid Status Code: 429".
    const DAILY_QUOTA: &str = r#"{"error":{"code":429,"message":"You exceeded your current quota","status":"RESOURCE_EXHAUSTED","details":[{"@type":"type.googleapis.com/google.rpc.QuotaFailure","violations":[{"quotaMetric":"generativelanguage.googleapis.com/generate_content_free_tier_requests","quotaId":"GenerateRequestsPerDayPerProjectPerModel-FreeTier","quotaValue":"20"}]},{"@type":"type.googleapis.com/google.rpc.RetryInfo","retryDelay":"59s"}]}}"#;
    let project = project_with_a_test();
    let model = ScriptedModel::start(vec![Turn::StatusBody(429, DAILY_QUOTA), Turn::Text("unreachable")]);
    let started = std::time::Instant::now();

    let run = run_exec(project.path(), &model, None);

    assert_eq!(run.exit_code, Some(1), "report: {}", run.report);
    assert_eq!(model.requests().len(), 1, "a daily quota must not be retried");
    assert_eq!(run.report["metrics"]["retried_llm_calls"], 0);
    let error = run.report["error"].as_str().unwrap();
    assert!(
        error.starts_with("provider quota exhausted (GenerateRequestsPerDayPerProjectPerModel-FreeTier)"),
        "{error}"
    );
    assert!(started.elapsed() < Duration::from_secs(20));
}

#[test]
fn test_parallel_reads_run_through_the_real_executor_and_keep_order() {
    // T2.1 end to end: the real tool executor, whose shared metrics and
    // read tracking must survive concurrent calls. The overwrite in the next
    // turn is refused unless the concurrent read of calc.py was recorded.
    let project = calc_project();
    let calc = project.path().join("calc.py");
    let test = project.path().join("tests/test_calc.py");
    let model = ScriptedModel::start(vec![
        Turn::Tools(vec![
            ("read", serde_json::json!({"file_path": calc})),
            ("read", serde_json::json!({"file_path": test})),
        ]),
        Turn::Tool(
            "write",
            serde_json::json!({"file_path": calc, "content": "def add(a, b):\n    return a + b\n", "overwrite": true}),
        ),
        Turn::Tool("shell", serde_json::json!({"command": CALC_TESTS, "description": "run tests"})),
        Turn::Text("Done."),
    ]);

    let run = run_exec_full(
        project.path(),
        &model,
        None,
        &[("PEACH_HARNESS_PARALLEL_READONLY", "1")],
        "fix add",
        &["--test-command", CALC_TESTS],
    );

    assert_eq!(run.exit_code, Some(0), "report: {}", run.report);
    let second: serde_json::Value = serde_json::from_str(&model.requests()[1]).unwrap();
    let tool_results: Vec<String> = second["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|message| message["role"] == "tool")
        .map(|message| message["tool_call_id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(tool_results, vec!["call_0", "call_1"], "results keep the model's order");
    assert_eq!(std::fs::read_to_string(&calc).unwrap(), "def add(a, b):\n    return a + b\n");
    let tools: Vec<(String, bool)> = run
        .telemetry
        .iter()
        .filter(|event| event["type"] == "tool_call")
        .map(|event| (event["name"].as_str().unwrap().to_string(), event["success"].as_bool().unwrap()))
        .collect();
    assert_eq!(
        tools,
        vec![
            ("read".to_string(), true),
            ("read".to_string(), true),
            ("write".to_string(), true),
            ("shell".to_string(), true)
        ]
    );
}

#[test]
fn test_misnamed_arguments_are_corrected_only_with_the_flag() {
    // T2.6: `filePath`/`contents` are not write's parameters. Without the
    // flag the strict parse rejects the call and the model must try again;
    // with it, the call goes through and the rename is recorded.
    let run = |flag: &str| {
        let project = calc_project();
        let target = project.path().join("notes.txt");
        let model = ScriptedModel::start(vec![
            Turn::Tool("write", serde_json::json!({"filePath": target, "contents": "hello"})),
            Turn::Text("Done."),
        ]);
        let run = run_exec_with_env(project.path(), &model, None, &[("PEACH_HARNESS_TOOL_CORRECTION", flag)]);
        (std::fs::read_to_string(&target).ok(), run)
    };

    let (without, _) = run("0");
    let (with, with_run) = run("1");

    assert_eq!(without, None, "the uncorrected call must fail as before");
    assert_eq!(with.as_deref(), Some("hello"));
    let renames: Vec<&str> = with_run
        .telemetry
        .iter()
        .filter(|event| event["type"] == "recovery" && event["action"] == "tool_argument_renamed")
        .map(|event| event["trigger"].as_str().unwrap())
        .collect();
    assert_eq!(
        renames,
        vec![
            "write: `contents` is not a parameter; used `content`",
            "write: `filePath` is not a parameter; used `file_path`"
        ]
    );
}

#[test]
fn test_an_edit_made_through_the_shell_also_needs_verifying() {
    // D-037's gap: only tool edits armed the gate, so `sed -i` then "Done."
    // ended the run unverified.
    let project = calc_project();
    let model = ScriptedModel::start(vec![
        Turn::Tool(
            "shell",
            serde_json::json!({"command": "sed -i.bak 's/a - b/a + b/' calc.py && rm calc.py.bak", "description": "fix"}),
        ),
        Turn::Text("Fixed."),
        Turn::Tool("shell", serde_json::json!({"command": CALC_TESTS, "description": "run tests"})),
        Turn::Text("Fixed and verified."),
    ]);

    let run = run_exec_full(project.path(), &model, None, &[], "fix add", &["--test-command", CALC_TESTS]);

    assert_eq!(run.exit_code, Some(0), "report: {}", run.report);
    let requests = model.requests();
    assert_eq!(requests.len(), 4, "sed edit, finish (sent back), test run, finish");
    assert!(requests[2].contains("before finishing (1 of 2)"));
    assert_eq!(std::fs::read_to_string(project.path().join("calc.py")).unwrap(), "def add(a, b):\n    return a + b\n");
}

#[test]
fn test_an_unattended_run_makes_no_title_request() {
    let project = project_with_a_test();
    let model = ScriptedModel::start(vec![Turn::Text("Done.")]);

    let run = run_exec(project.path(), &model, None);

    assert_eq!(run.exit_code, Some(0), "report: {}", run.report);
    assert_eq!(model.requests().len(), 1);
    assert_eq!(*model.side_requests.lock().unwrap(), 0, "no side request (title generation) in exec");
}


/// Config registering `fast` as a second provider serving `fast-model`, and
/// routing the `sage` role to it (MM.3, D-049).
fn sage_routed_to(fast: &ScriptedModel) -> String {
    format!(
        r#"
[[providers]]
id = "{provider}"
url = "{url}"
response_type = "OpenAI"
auth_methods = ["api_key"]
api_key_var = "PEACH_TEST_SCRIPTED_KEY"

[[providers.models]]
id = "fast-model"
name = "Fast"
tools_supported = true
input_modalities = ["text"]

[roles.sage]
provider_id = "{provider}"
model_id = "fast-model"
"#,
        provider = fast.provider_id,
        url = fast.url,
    )
}

fn delegate_to_sage() -> Turn {
    Turn::Tool(
        "task",
        serde_json::json!({"tasks": ["Where is add defined?"], "agent_id": "sage"}),
    )
}

#[test]
fn test_a_routed_subagent_runs_on_its_role_model_and_the_main_agent_does_not() {
    let project = project_with_a_test();
    let fast = ScriptedModel::start_as("scripted_fast", vec![Turn::Text("add is in calc.py")]);
    let main = ScriptedModel::start(vec![delegate_to_sage(), Turn::Text("Done.")]);

    let run = run_exec_configured(project.path(), &main, None, &[], "fix add", &[], &sage_routed_to(&fast));

    let model_of = |body: &String| {
        serde_json::from_str::<serde_json::Value>(body).unwrap()["model"].as_str().unwrap_or("").to_string()
    };
    let actual = (
        run.exit_code,
        fast.requests().iter().map(model_of).collect::<Vec<_>>(),
        main.requests().iter().map(model_of).collect::<std::collections::BTreeSet<_>>(),
    );
    let expected = (
        Some(0),
        vec!["fast-model".to_string()],
        ["scripted-model".to_string()].into_iter().collect(),
    );
    assert_eq!(actual, expected, "report: {}", run.report);
}

#[test]
fn test_a_failing_role_model_does_not_end_the_run() {
    let project = project_with_a_test();
    let fast = ScriptedModel::start_as(
        "scripted_fast",
        vec![Turn::Status(400), Turn::Status(400), Turn::Status(400), Turn::Status(400)],
    );
    let main = ScriptedModel::start(vec![delegate_to_sage(), Turn::Text("Done.")]);

    let run = run_exec_configured(project.path(), &main, None, &[], "fix add", &[], &sage_routed_to(&fast));

    let second = main.requests().get(1).cloned().unwrap_or_default();
    let delegated_result = serde_json::from_str::<serde_json::Value>(&second).unwrap()["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["role"] == "tool")
        .map(|m| m["content"].to_string())
        .collect::<String>();
    assert_eq!(run.exit_code, Some(0), "report: {}", run.report);
    assert!(
        delegated_result.contains("'sage' subagent's model (")
            && delegated_result.contains("/fast-model) failed")
            && delegated_result.contains("Continue the work yourself"),
        "the main agent was not told the role model failed: {delegated_result}"
    );
    assert!(
        run.telemetry.iter().any(|event| event["type"] == "recovery"
            && event["action"] == "subagent_model_failed"),
        "no recovery event: {:?}",
        run.telemetry
    );
}
