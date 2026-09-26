//! harness: R-HACK-3 — lifecycle hooks that feed the harness telemetry
//! stream (`peach_harness::telemetry`): one `model_call` per response, one
//! `tool_call` per tool result, `agent_state` at start and end, and (through
//! [`record_model_retry`], called from the orchestrator's retry notifier) one
//! `retry` per failed provider attempt.
//!
//! Every handler returns immediately when no sink is installed, which is
//! always the case outside `peach exec --telemetry`, so interactive use pays
//! nothing. Token counts come only from provider usage (HACKATHON.md §16);
//! the one local count is named `context_tokens_estimated`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use async_trait::async_trait;
use peach_domain::{
    Conversation, EndPayload, EventData, EventHandle, RequestPayload, ResponsePayload,
    StartPayload, TokenCount, ToolOutput, ToolValue, ToolcallEndPayload, ToolcallStartPayload,
};
use peach_harness::telemetry::event::Truncated;
use peach_harness::telemetry::{self, TelemetryEvent, event};

/// Emits telemetry events for one agent's conversation. Clones share state,
/// so the request and response chains can correlate a call's start and end.
#[derive(Clone, Default)]
pub struct TelemetryHandler {
    state: Arc<Mutex<State>>,
}

#[derive(Default)]
struct State {
    request: Option<PendingRequest>,
    model_calls: u64,
    last_model_call_id: Option<String>,
    tool_started: HashMap<String, Instant>,
}

struct PendingRequest {
    started: Instant,
    started_at: String,
    context_tokens_estimated: Option<u64>,
    context_messages: usize,
}

impl TelemetryHandler {
    /// Creates a handler with no calls in flight.
    pub fn new() -> Self {
        Self::default()
    }
}

fn now() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

/// A provider-reported count, or `None` for a local estimate (§16).
fn actual(count: TokenCount) -> Option<u64> {
    match count {
        TokenCount::Actual(value) => Some(value as u64),
        TokenCount::Approx(_) => None,
    }
}

/// The tool result as the model saw it, flattened to text.
fn output_text(output: &ToolOutput) -> String {
    output
        .values
        .iter()
        .map(|value| match value {
            ToolValue::Text(text) => text.as_str(),
            ToolValue::AI { value, .. } => value.as_str(),
            ToolValue::Image(_) => "[image]",
            ToolValue::Empty => "",
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn emit(event: TelemetryEvent, conversation: &Conversation, agent_id: &str) {
    telemetry::emit_with(event, Some(conversation.id.to_string()), Some(agent_id.to_string()));
}

/// Records one failed provider attempt that is about to be retried. Called
/// from the orchestrator's retry notifier, which exists whether or not a UI
/// is attached.
///
/// # Arguments
/// * `conversation_id` - Conversation the request belongs to.
/// * `agent_id` - Agent that made the request.
/// * `attempt` - 1 for the first retry.
/// * `max_attempts` - The configured retry ceiling.
/// * `error` - Why the attempt failed; an empty completion also carries any
///   usage the provider reported for it.
pub fn record_model_retry(
    conversation_id: &str,
    agent_id: &str,
    attempt: u64,
    max_attempts: usize,
    error: &anyhow::Error,
) {
    if !telemetry::is_installed() {
        return;
    }
    let billed = peach_domain::Error::billed_usage(error);
    let usage_reported = if billed.is_some() {
        Some(true)
    } else if is_empty_completion(error) {
        Some(false)
    } else {
        None
    };
    telemetry::emit_with(
        TelemetryEvent::Retry(event::Retry {
            operation: "model_call".to_string(),
            attempt: u32::try_from(attempt).unwrap_or(u32::MAX),
            max_attempts: u32::try_from(max_attempts).ok(),
            reason: error.root_cause().to_string(),
            origin_call_id: None,
            usage_reported,
            input_tokens: billed.and_then(|usage| actual(usage.prompt_tokens)),
            output_tokens: billed.and_then(|usage| actual(usage.completion_tokens)),
            reasoning_tokens: billed.and_then(|usage| actual(usage.reasoning_tokens)),
        }),
        Some(conversation_id.to_string()),
        Some(agent_id.to_string()),
    );
}

/// Whether `error` is an empty completion anywhere in its chain.
fn is_empty_completion(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| match cause.downcast_ref::<peach_domain::Error>() {
        Some(peach_domain::Error::EmptyCompletion { .. }) => true,
        Some(peach_domain::Error::Retryable(inner)) => is_empty_completion(inner),
        _ => false,
    })
}

#[async_trait]
impl EventHandle<EventData<StartPayload>> for TelemetryHandler {
    async fn handle(
        &self,
        event: &EventData<StartPayload>,
        conversation: &mut Conversation,
    ) -> anyhow::Result<()> {
        if telemetry::is_installed() {
            let state = event::AgentState {
                from: None,
                to: "running".to_string(),
                reason: None,
                iteration: None,
            };
            emit(TelemetryEvent::AgentState(state), conversation, event.agent.id.as_str());
        }
        Ok(())
    }
}

#[async_trait]
impl EventHandle<EventData<RequestPayload>> for TelemetryHandler {
    async fn handle(
        &self,
        _event: &EventData<RequestPayload>,
        conversation: &mut Conversation,
    ) -> anyhow::Result<()> {
        if !telemetry::is_installed() {
            return Ok(());
        }
        let (context_tokens_estimated, context_messages) = conversation
            .context
            .as_ref()
            .map(|context| (Some(context.token_count_approx() as u64), context.messages.len()))
            .unwrap_or((None, 0));
        if let Ok(mut state) = self.state.lock() {
            state.request = Some(PendingRequest {
                started: Instant::now(),
                started_at: now(),
                context_tokens_estimated,
                context_messages,
            });
        }
        Ok(())
    }
}

#[async_trait]
impl EventHandle<EventData<ResponsePayload>> for TelemetryHandler {
    async fn handle(
        &self,
        event: &EventData<ResponsePayload>,
        conversation: &mut Conversation,
    ) -> anyhow::Result<()> {
        if !telemetry::is_installed() {
            return Ok(());
        }
        let message = &event.payload.message;
        let Ok(mut state) = self.state.lock() else {
            return Ok(());
        };
        state.model_calls += 1;
        let call_id = format!("{}#{}", conversation.id, state.model_calls);
        state.last_model_call_id = Some(call_id.clone());
        let pending = state.request.take();
        drop(state);

        let (started_at, duration_ms, context_tokens_estimated, context_messages) = match pending {
            Some(pending) => (
                pending.started_at,
                pending.started.elapsed().as_millis() as u64,
                pending.context_tokens_estimated,
                pending.context_messages,
            ),
            None => (now(), 0, None, 0),
        };
        let usage = &message.usage;
        let model_call = event::ModelCall {
            call_id,
            started_at,
            ended_at: now(),
            duration_ms,
            model: event.model_id.to_string(),
            input_tokens: actual(usage.prompt_tokens),
            output_tokens: actual(usage.completion_tokens),
            total_tokens: actual(usage.total_tokens),
            cached_tokens: actual(usage.cached_tokens),
            reasoning_tokens: actual(usage.reasoning_tokens),
            context_tokens_estimated,
            context_messages,
            finish_reason: message
                .finish_reason
                .as_ref()
                .map(|reason| <&str>::from(reason).to_string()),
            tool_call_ids: message
                .tool_calls
                .iter()
                .filter_map(|call| call.call_id.as_ref().map(|id| id.as_str().to_string()))
                .collect(),
        };
        emit(TelemetryEvent::ModelCall(model_call), conversation, event.agent.id.as_str());
        Ok(())
    }
}

#[async_trait]
impl EventHandle<EventData<ToolcallStartPayload>> for TelemetryHandler {
    async fn handle(
        &self,
        event: &EventData<ToolcallStartPayload>,
        _conversation: &mut Conversation,
    ) -> anyhow::Result<()> {
        if !telemetry::is_installed() {
            return Ok(());
        }
        if let (Some(id), Ok(mut state)) = (&event.payload.tool_call.call_id, self.state.lock()) {
            state.tool_started.insert(id.as_str().to_string(), Instant::now());
        }
        Ok(())
    }
}

#[async_trait]
impl EventHandle<EventData<ToolcallEndPayload>> for TelemetryHandler {
    async fn handle(
        &self,
        event: &EventData<ToolcallEndPayload>,
        conversation: &mut Conversation,
    ) -> anyhow::Result<()> {
        if !telemetry::is_installed() {
            return Ok(());
        }
        let tool_call = &event.payload.tool_call;
        let result = &event.payload.result;
        let call_id = tool_call
            .call_id
            .as_ref()
            .map(|id| id.as_str().to_string())
            .unwrap_or_default();
        let (started, origin_call_id) = match self.state.lock() {
            Ok(mut state) => (state.tool_started.remove(&call_id), state.last_model_call_id.clone()),
            Err(_) => (None, None),
        };
        let text = output_text(&result.output);
        // Full text here; the sink redacts and then caps both fields.
        let tool = event::ToolCall {
            call_id,
            name: tool_call.name.to_string(),
            arguments: Truncated::whole(tool_call.arguments.to_owned().into_string()),
            success: !result.is_error(),
            duration_ms: started.map_or(0, |started| started.elapsed().as_millis() as u64),
            result_size_chars: text.chars().count(),
            result_summary: Truncated::whole(text),
            handle: None,
            origin_call_id,
        };
        emit(TelemetryEvent::ToolCall(tool), conversation, event.agent.id.as_str());
        Ok(())
    }
}

#[async_trait]
impl EventHandle<EventData<EndPayload>> for TelemetryHandler {
    async fn handle(
        &self,
        event: &EventData<EndPayload>,
        conversation: &mut Conversation,
    ) -> anyhow::Result<()> {
        if telemetry::is_installed() {
            let state = event::AgentState {
                from: Some("running".to_string()),
                to: "ended".to_string(),
                reason: None,
                iteration: self.state.lock().ok().map(|state| state.model_calls),
            };
            emit(TelemetryEvent::AgentState(state), conversation, event.agent.id.as_str());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use peach_domain::Image;
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_only_provider_reported_counts_are_kept() {
        let actual = (actual(TokenCount::Actual(42)), actual(TokenCount::Approx(42)));

        assert_eq!(actual, (Some(42), None));
    }

    #[test]
    fn test_tool_output_is_flattened_to_what_the_model_saw() {
        let fixture = ToolOutput::text("first").combine(ToolOutput::image(
            Image::new_base64("aGk=".to_string(), "image/png"),
        ));

        let actual = output_text(&fixture);

        assert_eq!(actual, "first\n[image]");
    }
}
