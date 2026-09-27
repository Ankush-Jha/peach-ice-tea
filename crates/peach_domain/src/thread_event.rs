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
    /// A message already in the view gained metadata without changing what
    /// it says: peach stamps the model on messages that lack one and attaches
    /// usage after a response. Recorded so the replayed view stays exact.
    Revise {
        /// Position of the message in the view.
        index: usize,
        /// The message as it is now. Boxed like `Message::entry`.
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
    /// The model wrote a scratchpad note (R-CTX-10, D-087). Notes live on the
    /// conversation's metrics, not in the view, so this changes neither
    /// history nor view; it keeps every note, including ones later evicted.
    Note {
        /// The note's id, from 1 in the order written.
        id: u64,
        /// The note as kept.
        text: String,
    },
}

impl ThreadEvent {
    /// A short, stable name for storage and logs.
    pub fn kind(&self) -> &'static str {
        match self {
            ThreadEvent::Message { .. } => "message",
            ThreadEvent::Revise { .. } => "revise",
            ThreadEvent::Compaction { .. } => "compaction",
            ThreadEvent::Note { .. } => "note",
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
/// the full history a compaction could not destroy. Each message appears as
/// it was first recorded; later metadata (`Revise`) is not applied, because
/// its index refers to the view, not the history.
///
/// # Arguments
/// * `events` - The conversation's events in `seq` order.
pub fn replay_history(events: &[ThreadEvent]) -> Vec<MessageEntry> {
    events
        .iter()
        .filter_map(|event| match event {
            ThreadEvent::Message { entry } => Some((**entry).clone()),
            ThreadEvent::Revise { .. }
            | ThreadEvent::Compaction { .. }
            | ThreadEvent::Note { .. } => None,
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
            ThreadEvent::Revise { index, entry } => {
                if let Some(slot) = view.get_mut(*index) {
                    *slot = (**entry).clone();
                }
            }
            ThreadEvent::Compaction { view: after, .. } => view = after.clone(),
            ThreadEvent::Note { .. } => {}
        }
        view
    })
}

/// The working view just before the first compaction at or after
/// `compaction_index` in `events`: the context exactly as it was when that
/// compaction ran (R-CTX-1's acceptance). All of `events` when there is none.
///
/// # Arguments
/// * `events` - The conversation's events in `seq` order.
/// * `compaction_index` - Where in `events` to start looking.
pub fn replay_view_before_compaction(
    events: &[ThreadEvent],
    compaction_index: usize,
) -> Vec<MessageEntry> {
    let end = events
        .iter()
        .enumerate()
        .skip(compaction_index)
        .find(|(_, event)| matches!(event, ThreadEvent::Compaction { .. }))
        .map_or(events.len(), |(at, _)| at);
    replay_view(events.get(..end).unwrap_or(events))
}

/// Whether two entries say the same thing: equal once the metadata peach adds
/// after the fact (the model stamped on text messages, usage) is ignored.
fn same_content(a: &MessageEntry, b: &MessageEntry) -> bool {
    let strip = |entry: &MessageEntry| {
        let mut entry = entry.clone();
        entry.usage = None;
        if let crate::ContextMessage::Text(text) = &mut entry.message {
            text.model = None;
        }
        entry
    };
    a == b || strip(a) == strip(b)
}

/// Marks the text of a tool result S1 replaced with a preview and a handle
/// (`compaction_pipeline::offload`), so the event log recognises the rewrite.
pub const OFFLOAD_STUB_MARKER: &str = "[offloaded by the harness:";

fn is_offload_stub(entry: &MessageEntry) -> bool {
    match &entry.message {
        crate::ContextMessage::Tool(result) => result.output.values.iter().any(|v| {
            v.as_str()
                .is_some_and(|t| t.starts_with(OFFLOAD_STUB_MARKER))
        }),
        _ => false,
    }
}

fn same_run(a: &[MessageEntry], b: &[MessageEntry]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| same_content(x, y))
}

/// The events that turn the view `before` into the view `after`, for a
/// writer that only sees snapshots (T3.2).
///
/// Messages are matched by content, ignoring the metadata peach adds after
/// the fact (see `Revise`). When `before` is still the start of `after`, the
/// changes are metadata revisions plus new `Message` events. Otherwise the
/// view was rewritten, which is a compaction, and one snapshot can hold a
/// compaction *and* messages appended after it, because peach compacts in
/// the middle of a turn and saves at its end. Peach's compactor replaces one
/// stretch with a single summary message, so after the prefix both views
/// share comes the summary, then the messages it kept from `before`, then
/// new ones: the `Compaction` view ends at the last kept message (or at the
/// summary when nothing was kept), and everything after it becomes new
/// `Message` events, which keeps them in the history.
///
/// # Arguments
/// * `before` - The view the log already reproduces.
/// * `after` - The view to record.
pub fn events_between(before: &[MessageEntry], after: &[MessageEntry]) -> Vec<ThreadEvent> {
    let messages = |entries: &[MessageEntry]| -> Vec<ThreadEvent> {
        entries
            .iter()
            .cloned()
            .map(|entry| ThreadEvent::Message { entry: Box::new(entry) })
            .collect()
    };
    // Bounds are checked by construction (`split_at_checked`, `zip`, `get`),
    // so no input can make this panic (CI's `indexing_slicing` lint).
    if let Some((head, tail)) = after.split_at_checked(before.len()) {
        if same_run(head, before) {
            let revisions = head
                .iter()
                .zip(before)
                .enumerate()
                .filter(|(_, (now, was))| now != was)
                .map(|(index, (now, _))| ThreadEvent::Revise {
                    index,
                    entry: Box::new(now.clone()),
                });
            return revisions.chain(messages(tail)).collect();
        }
        // An in-place rewrite (S1 offload replaces results with stubs, same
        // positions), possibly followed by new messages: the view is
        // `before`'s length of `after`, and the rest is new.
        if head
            .iter()
            .zip(before)
            .all(|(now, was)| same_content(now, was) || is_offload_stub(now))
        {
            let mut events = vec![ThreadEvent::Compaction {
                messages_before: before.len(),
                view: head.to_vec(),
            }];
            events.extend(messages(tail));
            return events;
        }
    }
    // Peach's compactor splices exactly one summary where the evicted stretch
    // began (`Compactor::compress_single_sequence`): after the common prefix,
    // the first entry is that summary, then whatever it kept from `before`,
    // then anything new.
    let prefix = before
        .iter()
        .zip(after)
        .take_while(|(b, a)| same_content(a, b))
        .count();
    let kept = before.get(prefix..).unwrap_or_default();
    let view_end = if prefix >= after.len() {
        after.len()
    } else {
        (prefix + 1..after.len())
            .rev()
            .find(|&i| {
                after
                    .get(i)
                    .is_some_and(|entry| kept.iter().any(|k| same_content(k, entry)))
            })
            .map_or(prefix + 1, |last_kept| last_kept + 1)
    };
    let (view, rest) = after.split_at_checked(view_end).unwrap_or((after, &[]));
    let mut events =
        vec![ThreadEvent::Compaction { messages_before: before.len(), view: view.to_vec() }];
    events.extend(messages(rest));
    events
}

/// Storage for conversation event logs and the artifacts large payloads are
/// moved to (R-CTX-1).
#[async_trait::async_trait]
pub trait ThreadEventRepository: Send + Sync {
    /// Appends `events` to the conversation's log, after its last event.
    ///
    /// # Errors
    /// Returns an error if storage fails; nothing is appended then.
    async fn append_events(
        &self,
        conversation_id: &ConversationId,
        events: Vec<ThreadEvent>,
    ) -> anyhow::Result<()>;

    /// The conversation's events in `seq` order; empty for a conversation
    /// that predates the log.
    ///
    /// # Errors
    /// Returns an error if storage fails or an event cannot be decoded.
    async fn list_events(
        &self,
        conversation_id: &ConversationId,
    ) -> anyhow::Result<Vec<StoredThreadEvent>>;
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
            .add_message(ContextMessage::assistant(
                "Reading it.",
                None,
                None,
                Some(vec![call]),
            ))
            .add_message(ContextMessage::tool_result(
                ToolResult::new(ToolName::new("read"))
                    .call_id(ToolCallId::new("c1"))
                    .success("def add(a, b): return a - b"),
            ))
            .add_message(ContextMessage::assistant("Found it.", None, None, None))
    }

    #[test]
    fn test_replaying_the_log_reproduces_the_pre_compaction_context_byte_for_byte() {
        let before = fixture_context();
        let summary =
            Context::default().add_message(ContextMessage::user("Summary: adder read.", None));
        let mut events = events_between(&[], &before.messages);
        events.extend(events_between(&before.messages, &summary.messages));

        let actual = (
            serde_json::to_string(&replay_view_before_compaction(&events, 0)).unwrap(),
            serde_json::to_string(&replay_history(&events)).unwrap(),
        );

        let expected = serde_json::to_string(&before.messages).unwrap();
        assert_eq!(actual, (expected.clone(), expected));
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
    fn test_messages_appended_after_a_compaction_in_the_same_snapshot_stay_in_the_history() {
        let before = fixture_context().messages;
        let summary: MessageEntry = ContextMessage::user("Summary.", None).into();
        let new_turn: MessageEntry =
            ContextMessage::assistant("Next step.", None, None, None).into();
        let after = vec![
            before[0].clone(),
            summary.clone(),
            before[3].clone(),
            new_turn.clone(),
        ];
        let mut events = events_between(&[], &before);
        events.extend(events_between(&before, &after));

        let actual = (
            events.iter().map(ThreadEvent::kind).collect::<Vec<_>>(),
            replay_view(&events),
            replay_history(&events).last().cloned(),
        );

        let expected = (
            vec![
                "message",
                "message",
                "message",
                "message",
                "compaction",
                "message",
            ],
            after,
            Some(new_turn),
        );
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_metadata_added_later_is_a_revision_not_a_compaction_and_the_view_stays_exact() {
        let before = fixture_context().messages;
        let mut stamped = before.clone();
        if let ContextMessage::Text(text) = &mut stamped[0].message {
            text.model = Some(crate::ModelId::new("m"));
        }
        let next: MessageEntry = ContextMessage::user("Next.", None).into();
        let mut after = stamped.clone();
        after.push(next);
        let mut events = events_between(&[], &before);
        events.extend(events_between(&before, &after));

        let actual = (
            events.iter().map(ThreadEvent::kind).collect::<Vec<_>>(),
            replay_view(&events),
        );

        let expected = (
            vec![
                "message", "message", "message", "message", "revise", "message",
            ],
            after,
        );
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_when_compaction_keeps_nothing_later_messages_are_still_split_out() {
        let before = fixture_context().messages;
        let summary: MessageEntry = ContextMessage::user("Summary.", None).into();
        let new_call: MessageEntry = ContextMessage::assistant("Calling.", None, None, None).into();
        let new_result: MessageEntry = ContextMessage::user("result", None).into();
        let after = vec![
            before[0].clone(),
            summary,
            new_call.clone(),
            new_result.clone(),
        ];

        let actual: Vec<&str> = events_between(&before, &after)
            .iter()
            .map(ThreadEvent::kind)
            .collect();

        assert_eq!(actual, vec!["compaction", "message", "message"]);
        assert_eq!(
            replay_history(&events_between(&before, &after)),
            vec![new_call, new_result]
        );
    }

    #[test]
    fn test_an_in_place_offload_plus_a_new_message_is_a_compaction_then_a_message() {
        let before = fixture_context().messages;
        let mut after = before.clone();
        after[2] = ContextMessage::tool_result(
            ToolResult::new(ToolName::new("read"))
                .call_id(ToolCallId::new("c1"))
                .success(format!("{OFFLOAD_STUB_MARKER} 27 chars of read]")),
        )
        .into();
        let new_turn: MessageEntry = ContextMessage::user("Next.", None).into();
        after.push(new_turn.clone());

        let events = events_between(&before, &after);

        let kinds: Vec<&str> = events.iter().map(ThreadEvent::kind).collect();
        assert_eq!(kinds, vec!["compaction", "message"]);
        assert_eq!(replay_history(&events), vec![new_turn]);
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
