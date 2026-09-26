//! Staged compaction (R-CTX-2, T3.4).
//!
//! Compaction runs as an ordered list of stages, cheapest and least lossy
//! first, stopping as soon as the context is under its budget. Today there is
//! one stage, `Summarize` (S3), which is peach's existing `Compactor`, so the
//! pipeline behaves exactly as the direct `Compactor` call it replaced (the
//! golden test below proves it). The cheaper stages - supersede (S0), offload
//! (S1) and relevance scoring (S2) - are `[A/B]` tasks (T3.5, T3.6, T3.9) and
//! join as new variants of [`Stage`] ahead of `Summarize`.

use peach_domain::{Compact, Context, Environment};

use crate::compact::Compactor;

/// One compaction stage.
pub enum Stage {
    /// S3: peach's summary frame over the eligible sequence (lossy, last
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
}

/// An ordered list of stages. Every pipeline ends with `Summarize`.
pub struct Pipeline {
    stages: Vec<Stage>,
    /// Token count at or below which later stages are skipped.
    target_tokens: usize,
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
        Self { stages: vec![Stage::Summarize(Compactor::new(compact, environment))], target_tokens }
    }

    /// Runs the stages in order until the context is at or below the target;
    /// the final stage always runs if reached.
    ///
    /// # Errors
    /// Returns the first stage error. The caller keeps the uncompacted
    /// context, as it did when `Compactor` failed.
    pub fn run(&self, mut context: Context) -> anyhow::Result<PipelineOutcome> {
        let mut stage_reached = "none";
        for (index, stage) in self.stages.iter().enumerate() {
            let is_last = index + 1 == self.stages.len();
            if !is_last && stage_reached != "none" && *context.token_count() <= self.target_tokens {
                break;
            }
            context = stage.apply(context)?;
            stage_reached = stage.name();
        }
        Ok(PipelineOutcome { context, stage_reached })
    }
}

#[cfg(test)]
mod tests {
    use peach_domain::{ContextMessage, ToolCallFull, ToolCallId, ToolName, ToolResult};
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
                .arguments(peach_domain::ToolCallArguments::from_json(&format!(r#"{{"file_path":"f{n}.py"}}"#)));
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
    fn test_the_final_stage_always_runs_and_is_reported() {
        let pipeline = Pipeline::new(Compact::new(), fixture_environment(), usize::MAX);

        let actual = pipeline.run(fixture_contexts().remove(2)).unwrap().stage_reached;

        assert_eq!(actual, "summarize");
    }
}
