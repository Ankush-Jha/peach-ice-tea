//! `ExternalScorer` (R-CTX-4, R-EXT-1; D-098): relevance scoring by an
//! outside command, so save-token-jev's Jev scorer, or any other, can plug
//! into S2 without the harness depending on it (D-003).
//!
//! Protocol `peach-ice-tea.scorer/1` (documented in
//! `docs/harness/SCORER_PROTOCOL.md`): the command gets one JSON request on
//! stdin and answers on stdout with save-token-jev's own answer shape,
//! `{"<id>": {"keep_call": p, "keep_result": p}}`, the two probability
//! questions of its `questionsFor`. The harness turns the probabilities into
//! decisions with the same threshold rule (`plan::decide`), and applies them
//! reversibly: a dropped result is offloaded to a readable file, not deleted.
//!
//! What the command sees has already been redacted and stripped of pinned
//! calls by `build_plan`. Anything wrong (spawn failure, non-zero exit,
//! timeout, malformed JSON, a probability outside 0–1) is an error, and
//! `build_plan` then keeps every call: the stage fails open (R-CTX-5), as
//! save-token-jev does on a malformed answer.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::RelevanceScorer;
use super::plan::{ResultStatus, ScoredCall, ScorerConfig, ToolCallSummary, decide};

/// The scorer command line, run through `/bin/sh -c`.
pub const ENV_VAR: &str = "PEACH_HARNESS_EXTERNAL_SCORER";

/// Seconds the command may take (default 30, R-CTX-5).
pub const TIMEOUT_ENV_VAR: &str = "PEACH_HARNESS_EXTERNAL_SCORER_TIMEOUT_SECS";

/// Identifies the request format, so a scorer can refuse one it does not know.
pub const PROTOCOL: &str = "peach-ice-tea.scorer/1";

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// One call as the scorer sees it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScorerCall {
    /// Identifier to answer under.
    pub id: String,
    /// The tool that ran.
    pub tool: String,
    /// Redacted preview of its arguments.
    pub input_preview: String,
    /// `ok` or `error`.
    pub status: &'static str,
    /// Size of its result.
    pub result_chars: usize,
    /// Position in the conversation.
    pub message_index: usize,
    /// Whether later text mentions its result.
    pub referenced_later: bool,
}

/// The request written to the command's stdin.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScorerRequest {
    /// Always [`PROTOCOL`].
    pub protocol: &'static str,
    /// The task, from the first user message.
    pub goal: String,
    /// The keep threshold the harness will apply to the answers.
    pub threshold: f32,
    /// The calls to score, pinned ones excluded.
    pub calls: Vec<ScorerCall>,
}

/// One answer, in save-token-jev's shape.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
struct Answer {
    keep_call: f32,
    keep_result: f32,
}

/// Scores with an outside command.
#[derive(Debug, Clone)]
pub struct ExternalScorer {
    command: String,
    timeout: Duration,
}

impl ExternalScorer {
    /// A scorer running `command` with a `timeout`.
    ///
    /// # Arguments
    /// * `command` - The command line, run through `/bin/sh -c`.
    /// * `timeout` - How long it may take.
    pub fn new(command: impl Into<String>, timeout: Duration) -> Self {
        Self { command: command.into(), timeout }
    }

    /// The scorer configured by [`ENV_VAR`], if any.
    pub fn from_env() -> Option<Self> {
        let command = std::env::var(ENV_VAR)
            .ok()
            .filter(|c| !c.trim().is_empty())?;
        let timeout = std::env::var(TIMEOUT_ENV_VAR)
            .ok()
            .and_then(|v| v.parse().ok())
            .map(Duration::from_secs)
            .unwrap_or(DEFAULT_TIMEOUT);
        Some(Self::new(command, timeout))
    }

    /// The request for `calls`.
    ///
    /// # Arguments
    /// * `calls` - The calls to score.
    /// * `goal` - The task.
    /// * `config` - The scorer config (for the threshold).
    pub fn request(calls: &[ToolCallSummary], goal: &str, config: &ScorerConfig) -> ScorerRequest {
        ScorerRequest {
            protocol: PROTOCOL,
            goal: goal.to_string(),
            threshold: config.threshold,
            calls: calls
                .iter()
                .map(|call| ScorerCall {
                    id: call.call_id.clone(),
                    tool: call.tool_name.clone(),
                    input_preview: call.input_preview.clone(),
                    status: match call.result_status {
                        ResultStatus::Ok => "ok",
                        ResultStatus::Error => "error",
                    },
                    result_chars: call.result_chars,
                    message_index: call.message_index,
                    referenced_later: call.referenced_later,
                })
                .collect(),
        }
    }

    fn run(&self, input: &[u8]) -> anyhow::Result<Vec<u8>> {
        let mut child = Command::new("/bin/sh")
            .args(["-c", &self.command])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        // Write stdin and read stdout on threads, so a scorer that answers
        // before reading everything (or a large request) cannot deadlock us.
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow::anyhow!("no stdin"))?;
        let input = input.to_vec();
        let writer = std::thread::spawn(move || stdin.write_all(&input));
        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("no stdout"))?;
        let reader = std::thread::spawn(move || {
            let mut out = Vec::new();
            stdout.read_to_end(&mut out).map(|_| out)
        });
        let started = Instant::now();
        let status = loop {
            if let Some(status) = child.try_wait()? {
                break status;
            }
            if started.elapsed() > self.timeout {
                let _ = child.kill();
                let _ = child.wait();
                anyhow::bail!("external scorer timed out after {:?}", self.timeout);
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        let _ = writer.join();
        let out = reader
            .join()
            .map_err(|_| anyhow::anyhow!("reader panicked"))??;
        anyhow::ensure!(status.success(), "external scorer exited with {status}");
        Ok(out)
    }
}

/// Parses the command's answer, rejecting anything malformed as a whole.
///
/// # Errors
/// Returns an error if the output is not a JSON object of
/// `{keep_call, keep_result}` pairs with probabilities in 0–1.
fn parse_answers(output: &[u8]) -> anyhow::Result<HashMap<String, Answer>> {
    let answers: HashMap<String, Answer> = serde_json::from_slice(output)?;
    for (id, answer) in &answers {
        for p in [answer.keep_call, answer.keep_result] {
            anyhow::ensure!(
                (0.0..=1.0).contains(&p),
                "answer for {id}: probability {p} outside 0-1"
            );
        }
    }
    Ok(answers)
}

impl RelevanceScorer for ExternalScorer {
    fn name(&self) -> &'static str {
        "external"
    }

    fn score(
        &self,
        calls: &[ToolCallSummary],
        goal: &str,
        config: &ScorerConfig,
    ) -> anyhow::Result<Vec<ScoredCall>> {
        let request = serde_json::to_vec(&Self::request(calls, goal, config))?;
        let answers = parse_answers(&self.run(&request)?)?;
        // Calls the scorer did not answer are left out; build_plan keeps them.
        Ok(calls
            .iter()
            .filter_map(|call| {
                let answer = answers.get(&call.call_id)?;
                let decision = decide(
                    Some(answer.keep_call),
                    Some(answer.keep_result),
                    false,
                    config,
                );
                Some(
                    ScoredCall::new(call.call_id.clone(), decision)
                        .keep_call(answer.keep_call)
                        .keep_result(answer.keep_result),
                )
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::scorer::build_plan;
    use crate::scorer::plan::Decision;

    fn fixture_calls() -> Vec<ToolCallSummary> {
        (1..=3)
            .map(|i| ToolCallSummary {
                call_id: format!("c{i}"),
                tool_name: "read".to_string(),
                input_preview: format!(
                    "{{\"file_path\":\"f{i}.py\",\"api_key\":\"sk-live-123456789012345678901234\"}}"
                ),
                result_status: ResultStatus::Ok,
                result_chars: 5000,
                message_index: i,
                referenced_later: false,
            })
            .collect()
    }

    fn config() -> ScorerConfig {
        ScorerConfig { preserve_recent_messages: 0, ..ScorerConfig::default() }
    }

    #[test]
    fn test_jev_shaped_answers_become_decisions_and_unanswered_calls_are_kept() {
        let scorer = ExternalScorer::new(
            r#"cat >/dev/null; echo '{"c1":{"keep_call":0.9,"keep_result":0.9},"c2":{"keep_call":0.8,"keep_result":0.1},"c3x":{"keep_call":0,"keep_result":0}}'"#,
            Duration::from_secs(10),
        );

        let actual: Vec<Decision> = build_plan(&scorer, &fixture_calls(), "fix it", &config())
            .scored
            .into_iter()
            .map(|s| s.decision)
            .collect();

        let expected = vec![
            Decision::Keep,
            Decision::Truncate { head_chars: 300 },
            Decision::Keep,
        ];
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_the_scorer_sees_redacted_input_and_the_protocol_name() {
        let dir = tempfile::tempdir().unwrap();
        let seen = dir.path().join("request.json");
        let scorer = ExternalScorer::new(
            format!("cat > {}; echo '{{}}'", seen.display()),
            Duration::from_secs(10),
        );

        build_plan(&scorer, &fixture_calls(), "fix it", &config());

        let request = std::fs::read_to_string(seen).unwrap();
        assert!(request.contains(PROTOCOL));
        assert!(
            !request.contains("sk-live-123456789012345678901234"),
            "a secret reached the scorer"
        );
    }

    #[test]
    fn test_a_broken_scorer_keeps_everything() {
        let broken = [
            ("exit 3", Duration::from_secs(10)),
            ("echo not json", Duration::from_secs(10)),
            (
                r#"echo '{"c1":{"keep_call":7,"keep_result":0}}'"#,
                Duration::from_secs(10),
            ),
            ("sleep 5", Duration::from_millis(200)),
        ];

        let actual: Vec<bool> = broken
            .iter()
            .map(|(command, timeout)| {
                build_plan(
                    &ExternalScorer::new(*command, *timeout),
                    &fixture_calls(),
                    "g",
                    &config(),
                )
                .scored
                .iter()
                .all(|s| s.decision == Decision::Keep)
            })
            .collect();

        assert_eq!(actual, vec![true, true, true, true]);
    }
}
