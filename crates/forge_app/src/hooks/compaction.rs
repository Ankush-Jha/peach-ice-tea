use async_trait::async_trait;
use forge_domain::{Agent, Conversation, Environment, EventData, EventHandle, ResponsePayload};
use tracing::{debug, info};

use crate::compaction_pipeline::Pipeline;

/// Hook handler that performs context compaction when needed
///
/// This handler checks if the conversation context has grown too large
/// and compacts it according to the agent's compaction configuration.
/// The handler mutates the conversation's context in-place if compaction
/// is triggered.
#[derive(Clone)]
pub struct CompactionHandler {
    agent: Agent,
    environment: Environment,
}

impl CompactionHandler {
    /// Creates a new compaction handler
    ///
    /// # Arguments
    /// * `agent` - The agent configuration containing compaction settings
    /// * `environment` - The environment configuration
    pub fn new(agent: Agent, environment: Environment) -> Self {
        Self { agent, environment }
    }
}

#[async_trait]
impl EventHandle<EventData<ResponsePayload>> for CompactionHandler {
    async fn handle(
        &self,
        _event: &EventData<ResponsePayload>,
        conversation: &mut Conversation,
    ) -> anyhow::Result<()> {
        if let Some(context) = &conversation.context {
            let token_count = context.token_count();
            if self.agent.compact.should_compact(context, *token_count) {
                info!(agent_id = %self.agent.id, "Compaction triggered by hook");
                // harness: R-CTX-2 (T3.4) — staged pipeline; today its only
                // stage is the `Compactor` this called directly before.
                let outcome = Pipeline::new(
                    self.agent.compact.clone(),
                    self.environment.clone(),
                    self.agent.compact.token_threshold.unwrap_or(0),
                )
                .run(context.clone())?;
                debug!(stage = outcome.stage_reached, "Compaction pipeline finished");
                let compacted = outcome.context;
                // harness: R-EVAL-2 — record what this compaction reclaimed.
                // The orchestrator's metrics sync deliberately preserves
                // `compactions`, because hooks are the only writer.
                let tokens_before = *token_count as u64;
                let tokens_after = *compacted.token_count() as u64;
                conversation
                    .metrics
                    .task
                    .compactions
                    .record(tokens_before, tokens_after);
                conversation.context = Some(compacted);
            } else {
                debug!(agent_id = %self.agent.id, "Compaction not needed");
            }
        }
        Ok(())
    }
}
