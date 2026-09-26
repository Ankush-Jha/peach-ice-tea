//! Staged compaction (R-CTX-2, T3.4).
//!
//! Compaction runs as an ordered list of stages, cheapest and least lossy
//! first, stopping as soon as the context is under its budget. Today there is
//! one stage, `Summarize` (S3), which is forge's existing `Compactor`, so the
//! pipeline behaves exactly as the direct `Compactor` call it replaced (the
//! golden test below proves it). The cheaper stages - supersede (S0), offload
//! (S1) and relevance scoring (S2) - are `[A/B]` tasks (T3.5, T3.6, T3.9) and
//! join as new variants of [`Stage`] ahead of `Summarize`.

pub mod handoff;
pub mod recall;

use forge_domain::{Compact, Context, ContextMessage, Environment};

use crate::compact::Compactor;

/// One compaction stage.
pub enum Stage {
    /// S3: forge's summary frame over the eligible sequence (lossy, last
    /// resort). Always the final stage.
    Summarize(Compactor),
}

impl Stage {
    /// A short, stable name for logs and metrics.
    pub fn name(&self) -> &'static str {
        match self {
            Stage::Summarize(_) => "summarize",
        }
    }

    fn apply(&self, context: Context) -> anyhow::Result<Context> {
        match self {
            Stage::Summarize(compactor) => compactor.compact(context, false),
        }
    }
}

/// What a pipeline run did.
#[derive(Debug, Clone, PartialEq)]
pub struct PipelineOutcome {
    /// The compacted context.
    pub context: Context,
    /// Name of the last stage that ran.
    pub stage_reached: &'static str,
    /// Files holding results the summary replaced (R-CTX-3), for the caller
    /// to register as recovery handles.
    pub recall_handles: Vec<recall::RecallHandle>,
}

/// An ordered list of stages. Every pipeline ends with `Summarize`.
pub struct Pipeline {
    stages: Vec<Stage>,
    /// Token count at or below which later stages are skipped.
    target_tokens: usize,
    /// Placed at the top of the summary S3 writes (R-CTX-6, [`handoff`]).
    handoff_note: Option<String>,
    /// Whether S3 keeps what it summarises away as recall handles ([`recall`]).
    recall_handles: bool,
}

impl Pipeline {
    /// The pipeline for an agent's compaction settings. With only S3
    /// available, this runs the existing `Compactor` once, as before.
    ///
    /// # Arguments
    /// * `compact` - The agent's compaction configuration.
    /// * `environment` - The environment the summary template renders with.
    /// * `target_tokens` - Stop once the context is at or below this size.
    pub fn new(compact: Compact, environment: Environment, target_tokens: usize) -> Self {
        Self {
            stages: vec![Stage::Summarize(Compactor::new(compact, environment))],
            target_tokens,
            handoff_note: None,
            recall_handles: false,
        }
    }

    /// Sets whether S3 writes recall handles for the results it replaces.
    pub fn recall_handles(mut self, enabled: bool) -> Self {
        self.recall_handles = enabled;
        self
    }

    /// Sets the handoff note placed at the top of the S3 summary.
    pub fn handoff_note(mut self, note: Option<String>) -> Self {
        self.handoff_note = note;
        self
    }

    /// Runs the stages in order until the context is at or below the target;
    /// the final stage always runs if reached.
    ///
    /// # Errors
    /// Returns the first stage error. The caller keeps the uncompacted
    /// context, as it did when `Compactor` failed.
    pub fn run(&self, mut context: Context) -> anyhow::Result<PipelineOutcome> {
        let mut stage_reached = "none";
        let mut recall_handles = Vec::new();
        for (index, stage) in self.stages.iter().enumerate() {
            let is_last = index + 1 == self.stages.len();
            if !is_last && stage_reached != "none" && *context.token_count() <= self.target_tokens {
                break;
            }
            let before = context.messages.clone();
            context = stage.apply(context)?;
            stage_reached = stage.name();
            if matches!(stage, Stage::Summarize(_)) {
                if self.recall_handles {
                    recall_handles = recall::write_handles(&before, &context.messages);
                    if let Some(section) = recall::recall_section(&recall_handles) {
                        edit_new_summary(&mut context, &before, |text| format!("{text}\n\n{section}"));
                    }
                }
                if let Some(note) = &self.handoff_note {
                    edit_new_summary(&mut context, &before, |text| format!("{note}\n\n{text}"));
                }
            }
        }
        Ok(PipelineOutcome { context, stage_reached, recall_handles })
    }
}

/// Rewrites the text of the one message S3 added: the summary is the
/// message that was not in the context before the stage ran.
fn edit_new_summary(context: &mut Context, before: &[forge_domain::MessageEntry], edit: impl Fn(&str) -> String) {
    let summary = context.messages.iter_mut().find(|entry| !before.contains(entry));
    if let Some(entry) = summary
        && let ContextMessage::Text(text) = &mut **entry
    {
        text.content = edit(&text.content);
    }
}

#[cfg(test)]
mod tests {
    use forge_domain::{ContextMessage, ToolCallFull, ToolCallId, ToolName, ToolResult};
    use pretty_assertions::assert_eq;

    use super::*;

    fn fixture_environment() -> Environment {
        use fake::{Fake, Faker};
        let env: Environment = Faker.fake();
        env.cwd(std::path::PathBuf::from("/test/working/dir"))
    }

    fn fixture_contexts() -> Vec<Context> {
        let tool_turn = |n: usize| {
            let call = ToolCallFull::new(ToolName::new("read"))
                .call_id(ToolCallId::new(format!("call_{n}")))
                .arguments(forge_domain::ToolCallArguments::from_json(&format!(r#"{{"file_path":"f{n}.py"}}"#)));
            vec![
                ContextMessage::assistant(format!("Reading f{n}.py"), None, None, Some(vec![call])),
                ContextMessage::tool_result(
                    ToolResult::new(ToolName::new("read"))
                        .call_id(ToolCallId::new(format!("call_{n}")))
                        .success(format!("contents of f{n}.py ").repeat(50)),
                ),
            ]
        };
        let long = (0..12).fold(Context::default().add_message(ContextMessage::user("Fix the bug.", None)), |ctx, n| {
            tool_turn(n).into_iter().fold(ctx, |ctx, m| ctx.add_message(m))
        });
        vec![
            Context::default(),
            Context::default().add_message(ContextMessage::user("hello", None)),
            long.clone(),
            long.add_message(ContextMessage::user("Now also add a test.", None))
                .add_message(ContextMessage::assistant("Done.", None, None, None)),
        ]
    }

    #[test]
    fn test_the_pipeline_is_identical_to_the_compactor_it_replaced() {
        let compact = Compact::new().retention_window(2usize).eviction_window(0.5);
        let environment = fixture_environment();
        let pipeline = Pipeline::new(compact.clone(), environment.clone(), 0);
        let compactor = Compactor::new(compact, environment);

        let mut compacted = 0;
        for fixture in fixture_contexts() {
            let actual = pipeline.run(fixture.clone()).unwrap().context;
            let expected = compactor.compact(fixture.clone(), false).unwrap();
            compacted += usize::from(expected != fixture);
            assert_eq!(actual, expected);
        }
        // Guards the guard: identical no-ops would prove nothing.
        assert!(compacted >= 2, "only {compacted} fixture(s) were actually compacted");
    }

    #[test]
    fn test_the_handoff_note_tops_the_summary_and_nothing_else_changes() {
        let compact = Compact::new().retention_window(2usize).eviction_window(0.5);
        let fixture = fixture_contexts().remove(2);
        let plain = Pipeline::new(compact.clone(), fixture_environment(), 0).run(fixture.clone()).unwrap().context;

        let actual = Pipeline::new(compact, fixture_environment(), 0)
            .handoff_note(Some("HANDOFF NOTE test".to_string()))
            .run(fixture.clone())
            .unwrap()
            .context;

        let changed: Vec<usize> = (0..plain.messages.len()).filter(|&i| plain.messages[i] != actual.messages[i]).collect();
        assert_eq!(changed.len(), 1, "exactly the summary should differ");
        let ContextMessage::Text(text) = &*actual.messages[changed[0]] else { panic!("summary is not text") };
        let ContextMessage::Text(plain_text) = &*plain.messages[changed[0]] else { panic!("summary is not text") };
        assert_eq!(text.content, format!("HANDOFF NOTE test\n\n{}", plain_text.content));
    }

    #[test]
    fn test_recall_handles_list_every_summarised_result_below_the_summary() {
        let compact = Compact::new().retention_window(2usize).eviction_window(0.5);
        let fixture = fixture_contexts().remove(2);

        let outcome = Pipeline::new(compact, fixture_environment(), 0).recall_handles(true).run(fixture).unwrap();

        let summary = outcome
            .context
            .messages
            .iter()
            .find_map(|entry| match &**entry {
                ContextMessage::Text(text) if text.content.contains("RECOVERABLE RESULTS") => Some(text.content.clone()),
                _ => None,
            })
            .expect("no recall section in the summary");
        assert!(!outcome.recall_handles.is_empty());
        for handle in &outcome.recall_handles {
            assert!(summary.contains(&handle.path.display().to_string()));
            assert!(std::fs::read_to_string(&handle.path).unwrap().contains("contents of f"));
            let _ = std::fs::remove_file(&handle.path);
        }
    }

    #[test]
    fn test_the_final_stage_always_runs_and_is_reported() {
        let pipeline = Pipeline::new(Compact::new(), fixture_environment(), usize::MAX);

        let actual = pipeline.run(fixture_contexts().remove(2)).unwrap().stage_reached;

        assert_eq!(actual, "summarize");
    }
}
