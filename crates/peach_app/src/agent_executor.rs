use std::sync::Arc;

use anyhow::Context;
use convert_case::{Case, Casing};
use peach_domain::{
    AgentId, ChatRequest, ChatResponse, ChatResponseContent, Conversation, ConversationId, Event,
    TitleFormat, ToolCallContext, ToolDefinition, ToolName, ToolOutput,
};
use peach_template::Element;
use futures::StreamExt;
use tokio::sync::RwLock;

use crate::error::Error;
use crate::{AgentRegistry, ConversationService, EnvironmentInfra, Services};
#[derive(Clone)]
pub struct AgentExecutor<S> {
    services: Arc<S>,
    pub tool_agents: Arc<RwLock<Option<Vec<ToolDefinition>>>>,
}

impl<S: Services + EnvironmentInfra<Config = peach_config::PeachConfig>> AgentExecutor<S> {
    pub fn new(services: Arc<S>) -> Self {
        Self { services, tool_agents: Arc::new(RwLock::new(None)) }
    }

    /// Returns a list of tool definitions for all available agents.
    pub async fn agent_definitions(&self) -> anyhow::Result<Vec<ToolDefinition>> {
        if let Some(tool_agents) = self.tool_agents.read().await.clone() {
            return Ok(tool_agents);
        }
        let agents = self.services.get_agents().await?;
        let tools: Vec<ToolDefinition> = agents.into_iter().map(Into::into).collect();
        *self.tool_agents.write().await = Some(tools.clone());
        Ok(tools)
    }

    /// Executes an agent tool call by creating a new chat request for the
    /// Executes an agent tool call by creating a new chat request for the
    /// specified agent. If conversation_id is provided, the agent will reuse
    /// that conversation, maintaining context across invocations. Otherwise,
    /// a new conversation is created.
    pub async fn execute(
        &self,
        agent_id: AgentId,
        task: String,
        ctx: &ToolCallContext,
        conversation_id: Option<ConversationId>,
    ) -> anyhow::Result<ToolOutput> {
        ctx.send_tool_input(
            TitleFormat::debug(format!(
                "{} [Agent]",
                agent_id.as_str().to_case(Case::UpperSnake)
            ))
            .sub_title(task.as_str()),
        )
        .await?;

        // Reuse existing conversation if provided, otherwise create a new one
        let conversation = if let Some(conversation_id) = conversation_id {
            self.services
                .conversation_service()
                .find_conversation(&conversation_id)
                .await?
                .ok_or(Error::ConversationNotFound { id: conversation_id })?
        } else {
            // Create context with agent initiator since it's spawned by a
            // parent agent This is crucial for GitHub Copilot
            // billing optimization
            let context = peach_domain::Context::default().initiator("agent".to_string());
            let conversation = Conversation::generate()
                .title(task.clone())
                .context(context.clone());
            self.services
                .conversation_service()
                .upsert_conversation(conversation.clone())
                .await?;
            conversation
        };
        // Execute the request through the PeachApp
        let app = crate::PeachApp::new(self.services.clone());
        let mut response_stream = app
            .chat(
                agent_id.clone(),
                ChatRequest::new(Event::new(task.clone()), conversation.id),
            )
            .await?;

        // Collect responses from the agent
        let mut output = String::new();
        while let Some(message) = response_stream.next().await {
            let message = message?;
            if matches!(
                &message,
                ChatResponse::ToolCallStart { .. } | ChatResponse::ToolCallEnd(_)
            ) {
                output.clear();
            }
            match message {
                ChatResponse::TaskMessage { ref content } => match content {
                    ChatResponseContent::ToolInput(_) => ctx.send(message).await?,
                    ChatResponseContent::ToolOutput(_) => {}
                    ChatResponseContent::Markdown { text, partial } => {
                        if *partial {
                            output.push_str(text);
                        } else {
                            output = text.to_string();
                        }
                    }
                },
                ChatResponse::TaskReasoning { .. } => {}
                ChatResponse::TaskComplete => {}
                ChatResponse::ToolCallStart { .. } => ctx.send(message).await?,
                ChatResponse::ToolCallEnd(_) => ctx.send(message).await?,
                ChatResponse::RetryAttempt { .. } => ctx.send(message).await?,
                ChatResponse::Interrupt { reason } => {
                    return Err(Error::AgentToolInterrupted(reason))
                        .context(format!(
                            "Tool call to '{}' failed.\n\
                             Note: This is an AGENTIC tool (powered by an LLM), not a traditional function.\n\
                             The failure occurred because the underlying LLM did not behave as expected.\n\
                             This is typically caused by model limitations, prompt issues, or reaching safety limits.",
                            agent_id.as_str()
                        ));
                }
            }
        }
        // harness: R-EVAL-2 — fold the subagent's costs into the parent task.
        // The subagent ran on its own Conversation, so its tokens and tool
        // calls landed on that conversation's metrics. `PeachApp::chat`
        // persists it once the run completes, which has happened by the time
        // the stream closes, so re-read it here. Failing to look it up must not
        // fail the tool call: metrics are observational, and losing them is far
        // better than losing the subagent's actual work.
        match self
            .services
            .conversation_service()
            .find_conversation(&conversation.id)
            .await
        {
            Ok(Some(finished)) => {
                ctx.with_metrics(|metrics| {
                    metrics.task.absorb_subagent(&finished.metrics.task);
                })?;
            }
            Ok(None) => {
                tracing::debug!(
                    conversation_id = %conversation.id,
                    "Subagent conversation not found; its cost is not counted"
                );
            }
            Err(error) => {
                tracing::debug!(
                    conversation_id = %conversation.id,
                    ?error,
                    "Could not read subagent metrics; its cost is not counted"
                );
            }
        }

        if !output.is_empty() {
            // Create tool output
            Ok(ToolOutput::ai(
                conversation.id,
                Element::new("task_completed")
                    .attr("task", &task)
                    .append(Element::new("output").text(output)),
            ))
        } else {
            Err(Error::EmptyToolResponse.into())
        }
    }

    pub async fn contains_tool(&self, tool_name: &ToolName) -> anyhow::Result<bool> {
        let agent_tools = self.agent_definitions().await?;
        Ok(agent_tools.iter().any(|tool| tool.name == *tool_name))
    }
}
