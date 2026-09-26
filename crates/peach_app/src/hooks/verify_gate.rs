//! harness: R-HACK-7 — verified completion, on the peach side.
//!
//! [`observe`] feeds `peach_harness::verify` from the tool executor, where
//! the structured result (exit code included) is still available.
//! [`VerifyGateHandler`] asks the agent to verify before a run ends on an
//! unverified edit. Both do nothing unless an `exec` runtime is installed.

use std::time::Duration;

use async_trait::async_trait;
use peach_domain::{ContextMessage, Conversation, EndPayload, EventData, EventHandle, Role};
use peach_template::Element;

use crate::operation::ToolOperation;

/// Records a successful tool operation for verification: source edits arm
/// the completion gate, and test runs are classified. Returns a recovery
/// hint to append to the tool's result, when there is one.
pub fn observe(operation: &ToolOperation, duration: Duration) -> Option<String> {
    peach_harness::runtime::get()?;
    match operation {
        ToolOperation::FsWrite { .. }
        | ToolOperation::FsRemove { .. }
        | ToolOperation::FsPatch { .. }
        | ToolOperation::FsMultiPatch { .. }
        | ToolOperation::FsUndo { .. } => {
            peach_harness::verify::record_edit();
            None
        }
        ToolOperation::Shell { output } => {
            let result = &output.output;
            let combined = format!("{}\n{}", result.stdout, result.stderr);
            peach_harness::verify::record_shell(
                &result.command,
                result.exit_code,
                &combined,
                duration.as_millis() as u64,
            )
        }
        _ => None,
    }
}

/// Keeps an unattended run from ending on an unverified edit (End hook).
#[derive(Clone, Default)]
pub struct VerifyGateHandler;

impl VerifyGateHandler {
    /// Creates the handler.
    pub fn new() -> Self {
        Self
    }
}

/// Whether the last message is the assistant stopping of its own accord.
/// End also fires when a request or tool-failure limit stops the loop; the
/// gate must never keep a run going past its limits.
pub(crate) fn stopped_by_choice(conversation: &Conversation) -> bool {
    conversation
        .context
        .as_ref()
        .and_then(|context| context.messages.last())
        .is_some_and(|entry| match &entry.message {
            ContextMessage::Text(text) => {
                text.role == Role::Assistant && text.tool_calls.as_ref().is_none_or(Vec::is_empty)
            }
            _ => false,
        })
}

#[async_trait]
impl EventHandle<EventData<EndPayload>> for VerifyGateHandler {
    async fn handle(
        &self,
        _event: &EventData<EndPayload>,
        conversation: &mut Conversation,
    ) -> anyhow::Result<()> {
        if !stopped_by_choice(conversation) {
            return Ok(());
        }
        if let (Some(message), Some(context)) =
            (peach_harness::verify::gate_message(), conversation.context.as_mut())
        {
            let content = Element::new("system_reminder").text(message);
            context.messages.push(ContextMessage::user(content, None).into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use peach_domain::{Context, ToolCallFull, ToolName};

    use super::*;

    fn fixture(message: ContextMessage) -> Conversation {
        let mut conversation = Conversation::generate();
        conversation.context = Some(Context::default().add_message(message));
        conversation
    }

    #[test]
    fn test_only_a_voluntary_stop_counts() {
        let done = fixture(ContextMessage::assistant("Done.", None, None, None));
        let mid_tool = fixture(ContextMessage::assistant(
            "",
            None,
            None,
            Some(vec![ToolCallFull::new(ToolName::new("shell"))]),
        ));
        let user_last = fixture(ContextMessage::user("hi", None));

        let actual = (stopped_by_choice(&done), stopped_by_choice(&mid_tool), stopped_by_choice(&user_last));

        assert_eq!(actual, (true, false, false));
    }
}
