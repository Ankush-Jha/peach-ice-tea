//! The append-only event log of a conversation (R-CTX-1, T3.1).
//!
//! `conversations.context` is the working view, and compaction overwrites it:
//! once a summary replaces a stretch of turns, the turns are gone. The event
//! log keeps every message ever added, and every compaction as an event that
//! records the new view, so nothing is deleted and the working view is a
//! projection that can always be rebuilt. Reversible compaction (T3.5, T3.6)
//! and `recall` (T3.3) read their originals from here.

use serde::{Deserialize, Serialize};

use crate::{ConversationId, MessageEntry};

/// One entry in a conversation's event log.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ThreadEvent {
    /// A message joined the context: user, assistant (with its reasoning and
    /// tool calls) or tool result, exactly as it entered the context.
    Message {
        /// The context entry, byte for byte. Boxed so `Compaction` events
        /// are not padded to its size; serialisation is unaffected.
        entry: Box<MessageEntry>,
    },
    /// The working view was compacted. `view` is the context's messages
    /// straight after; the messages it replaced stay in the log.
    Compaction {
        /// How many messages the view had before compacting.
        messages_before: usize,
        /// The view's messages after compacting.
        view: Vec<MessageEntry>,
    },
}

impl ThreadEvent {
    /// A short, stable name for storage and logs.
    pub fn kind(&self) -> &'static str {
        match self {
            ThreadEvent::Message { .. } => "message",
            ThreadEvent::Compaction { .. } => "compaction",
        }
    }
}

/// An event as stored: its position in the conversation's log.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredThreadEvent {
    /// The conversation the event belongs to.
    pub conversation_id: ConversationId,
    /// Position in that conversation's log, from 1, without gaps.
    pub seq: u64,
    /// The event.
    pub event: ThreadEvent,
}

/// Every message the conversation ever had, in order, ignoring compactions:
/// the full history a compaction could not destroy.
///
/// # Arguments
/// * `events` - The conversation's events in `seq` order.
pub fn replay_history(events: &[ThreadEvent]) -> Vec<MessageEntry> {
    events
        .iter()
        .filter_map(|event| match event {
            ThreadEvent::Message { entry } => Some((**entry).clone()),
            ThreadEvent::Compaction { .. } => None,
        })
        .collect()
}

/// The working view the events produce: messages appended in order, each
/// compaction replacing the view with the one it recorded.
///
/// # Arguments
/// * `events` - The conversation's events in `seq` order.
pub fn replay_view(events: &[ThreadEvent]) -> Vec<MessageEntry> {
    events.iter().fold(Vec::new(), |mut view, event| {
        match event {
            ThreadEvent::Message { entry } => view.push((**entry).clone()),
            ThreadEvent::Compaction { view: after, .. } => view = after.clone(),
        }
        view
    })
}

/// The events that turn the view `before` into the view `after`, for a
/// writer that only sees snapshots (T3.2): new messages appended at the end
/// become `Message` events; anything else is a rewrite of earlier messages,
/// which is what compaction does, and becomes one `Compaction` event.
///
/// # Arguments
/// * `before` - The view the log already reproduces.
/// * `after` - The view to record.
pub fn events_between(before: &[MessageEntry], after: &[MessageEntry]) -> Vec<ThreadEvent> {
    if after.len() >= before.len() && after[..before.len()] == *before {
        after[before.len()..]
            .iter()
            .cloned()
            .map(|entry| ThreadEvent::Message { entry: Box::new(entry) })
            .collect()
    } else {
        vec![ThreadEvent::Compaction { messages_before: before.len(), view: after.to_vec() }]
    }
}

/// Storage for conversation event logs and the artifacts large payloads are
/// moved to (R-CTX-1).
#[async_trait::async_trait]
pub trait ThreadEventRepository: Send + Sync {
    /// Appends `events` to the conversation's log, after its last event.
    ///
    /// # Errors
    /// Returns an error if storage fails; nothing is appended then.
    async fn append_events(&self, conversation_id: &ConversationId, events: Vec<ThreadEvent>) -> anyhow::Result<()>;

    /// The conversation's events in `seq` order; empty for a conversation
    /// that predates the log.
    ///
    /// # Errors
    /// Returns an error if storage fails or an event cannot be decoded.
    async fn list_events(&self, conversation_id: &ConversationId) -> anyhow::Result<Vec<StoredThreadEvent>>;
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{Context, ContextMessage, ToolCallFull, ToolCallId, ToolName, ToolResult};

    fn fixture_context() -> Context {
        let call = ToolCallFull::new(ToolName::new("read")).call_id(ToolCallId::new("c1"));
        Context::default()
            .add_message(ContextMessage::user("Fix the adder.", None))
            .add_message(ContextMessage::assistant("Reading it.", None, None, Some(vec![call])))
            .add_message(ContextMessage::tool_result(
                ToolResult::new(ToolName::new("read")).call_id(ToolCallId::new("c1")).success("def add(a, b): return a - b"),
            ))
            .add_message(ContextMessage::assistant("Found it.", None, None, None))
    }

    #[test]
    fn test_replaying_the_log_reproduces_the_pre_compaction_context_byte_for_byte() {
        let before = fixture_context();
        let summary = Context::default().add_message(ContextMessage::user("Summary: adder read.", None));
        let mut events = events_between(&[], &before.messages);
        events.extend(events_between(&before.messages, &summary.messages));

        let actual = serde_json::to_string(&replay_history(&events)).unwrap();

        let expected = serde_json::to_string(&before.messages).unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_the_view_follows_compactions_and_later_messages() {
        let before = fixture_context();
        let compacted = vec![before.messages[0].clone(), before.messages[3].clone()];
        let later = ContextMessage::user("Now add a test.", None);
        let mut after = compacted.clone();
        after.push(later.clone().into());
        let mut events = events_between(&[], &before.messages);
        events.extend(events_between(&before.messages, &compacted));
        events.extend(events_between(&compacted, &after));

        let actual = (replay_view(&events), replay_history(&events).len());

        let expected = (after, 5);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_appending_is_messages_and_rewriting_is_one_compaction() {
        let context = fixture_context();

        let actual: Vec<&str> = events_between(&context.messages[..2], &context.messages)
            .iter()
            .chain(events_between(&context.messages, &context.messages[1..]).iter())
            .map(ThreadEvent::kind)
            .collect();

        assert_eq!(actual, vec!["message", "message", "compaction"]);
    }
}
