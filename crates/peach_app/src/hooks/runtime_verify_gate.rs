//! harness: R-HACK-10 — the runtime verification gate.
//!
//! Unlike `VerifyGateHandler` (which asks the model to verify and trusts its
//! report), this hook has the harness itself run the repository's test
//! command before a voluntary stop is accepted as completion. On failure,
//! the real output goes back into the conversation as the next turn's
//! input and the run continues; on a pass, completion is allowed. Config-
//! gated (`PeachConfig::runtime_verify_gate`, default off — CLAUDE.md
//! principle 6) and fails open with no test command configured or no `exec`
//! runtime installed (principle 5). Never touches the test-integrity guard's
//! own refusal/restore path — it only re-verifies through
//! `peach_harness::runtime::restore_integrity_if_installed`, the same
//! mechanism `ExecHarness::finish` already uses after its final run.

use std::time::Duration;

use async_trait::async_trait;
use peach_domain::{ContextMessage, Conversation, EndPayload, EventData, EventHandle};
use peach_harness::verify::FailureClass;
use peach_template::Element;

use crate::hooks::verify_gate::stopped_by_choice;

/// Wall-clock limit for one gate run. Shorter than the true final run's
/// `FINAL_TEST_TIMEOUT_SECS` (300s, in `harness_exec.rs`) because this can
/// run more than once per task.
const RUNTIME_GATE_TIMEOUT_SECS: u64 = 180;

fn runtime_gate_timeout() -> Duration {
    std::env::var("PEACH_HARNESS_RUNTIME_GATE_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.parse().ok())
        .map(Duration::from_secs)
        .unwrap_or(Duration::from_secs(RUNTIME_GATE_TIMEOUT_SECS))
}

/// Runs the repository's real test command before a voluntary stop is
/// accepted as completion (config-gated, R-HACK-10).
#[derive(Clone, Default)]
pub struct RuntimeVerifyGateHandler;

impl RuntimeVerifyGateHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl EventHandle<EventData<EndPayload>> for RuntimeVerifyGateHandler {
    async fn handle(
        &self,
        _event: &EventData<EndPayload>,
        conversation: &mut Conversation,
    ) -> anyhow::Result<()> {
        // Never override a request/tool-failure-limit stop — only a genuine
        // voluntary stop is a "done" claim to verify.
        if !stopped_by_choice(conversation) {
            return Ok(());
        }
        let Some(runtime) = peach_harness::runtime::get() else { return Ok(()) };
        if !runtime.is_non_interactive() {
            return Ok(());
        }
        // Fail open (principle 5): nothing to run against.
        let Some(test) = runtime.test_command().cloned() else { return Ok(()) };

        let (attempt, exhausted) = peach_harness::verify::runtime_gate_attempt();
        if exhausted {
            peach_harness::telemetry::emit(peach_harness::telemetry::TelemetryEvent::AgentState(
                peach_harness::telemetry::event::AgentState {
                    from: None,
                    to: "runtime_verify_gate_exhausted".to_string(),
                    reason: Some(format!(
                        "the harness's own `{}` run failed {} time(s) in a row; allowing the run to end \
                         rather than loop forever",
                        test.command,
                        peach_harness::verify::MAX_RUNTIME_GATE_ATTEMPTS
                    )),
                    iteration: None,
                },
            ));
            return Ok(());
        }

        let root = runtime.repo_root().to_path_buf();
        let timeout = runtime_gate_timeout();
        let test_for_run = test.clone();
        let run = tokio::task::spawn_blocking(move || {
            peach_harness::verify::run_gate(&root, &test_for_run, timeout)
        })
        .await?;

        // The suite may have written files of its own; never let that stand
        // as an unreviewed change to a protected test (same guarantee
        // `ExecHarness::finish` gives the true final run).
        peach_harness::runtime::restore_integrity_if_installed();

        if run.class == FailureClass::Passed {
            return Ok(());
        }

        let hint = peach_harness::verify::recovery_hint(run.class, Some(&test)).unwrap_or_default();
        let message = format!(
            "RUNTIME VERIFICATION GATE (harness, attempt {attempt} of {}): the harness ran `{}` itself, \
             just now — this is the command's real result, not your report of one. It did not pass: {}{}\
             (exit {:?}).\n\nLast output:\n{}\n\n{hint}\nDo not claim the task is finished again until this \
             command exits 0. Fix the source code, never the tests, then continue.",
            peach_harness::verify::MAX_RUNTIME_GATE_ATTEMPTS,
            test.command,
            run.class.as_str(),
            if run.timed_out { " (timed out)" } else { "" },
            run.exit_code,
            run.output_tail,
        );

        if let Some(context) = conversation.context.as_mut() {
            let content = Element::new("system_reminder").text(message);
            context.messages.push(ContextMessage::user(content, None).into());
        }
        Ok(())
    }
}
