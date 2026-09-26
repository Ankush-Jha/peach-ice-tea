//! harness: R-CTX-10 (D-087) — shows the model's scratchpad notes again
//! whenever compaction has removed them from view.
//!
//! Notes live on the conversation's metrics, not in the context (see
//! `forge_domain::scratchpad`). A note is in view while its `write_note`
//! result, or an earlier scratchpad reminder, is still in the context; both
//! carry the note's `[note N]` label. Once a summary has replaced every
//! message carrying some note's label, this re-appends all the notes,
//! verbatim, as one reminder. Checking what is in view rather than what
//! changed means a note costs its tokens once while it is in view, not
//! again after every `write_note` (principle 1).
//!
//! Runs on the response hook, after `CompactionHandler`, so the reminder
//! reaches the very next request after a compaction.

use async_trait::async_trait;
use forge_domain::{
    ContextMessage, Conversation, EventData, EventHandle, MessageEntry, ResponsePayload,
    ScratchNote,
};
use forge_template::Element;

/// Environment variable that enables `write_note` and this hook when `1`.
pub const ENV_VAR: &str = "FORGE_HARNESS_WRITE_NOTE";

/// Whether the scratchpad (`write_note` and its reminders) is enabled.
pub fn enabled() -> bool {
    std::env::var(ENV_VAR).is_ok_and(|value| value == "1")
}

/// First line of every scratchpad reminder.
const HEADER: &str = "SCRATCHPAD — notes you saved with write_note. Earlier messages were summarised, \
                      so they are repeated here word for word:";

/// Re-appends the scratchpad when some note is no longer in view.
#[derive(Debug, Clone, Default)]
pub struct NotesHandler;

impl NotesHandler {
    /// Creates a notes handler.
    pub fn new() -> Self {
        Self
    }
}

/// Whether `entry`'s text mentions `label`, in a text message or a tool
/// result.
fn mentions(entry: &MessageEntry, label: &str) -> bool {
    match &entry.message {
        ContextMessage::Tool(result) => result
            .output
            .values
            .iter()
            .any(|value| value.as_str().is_some_and(|text| text.contains(label))),
        message => message.content().is_some_and(|text| text.contains(label)),
    }
}

/// The scratchpad reminder listing `notes`.
fn reminder(notes: &[ScratchNote]) -> String {
    let lines: Vec<String> = notes.iter().map(|note| format!("{} {}", note.label(), note.text)).collect();
    format!("{HEADER}\n{}", lines.join("\n"))
}

#[async_trait]
impl EventHandle<EventData<ResponsePayload>> for NotesHandler {
    async fn handle(
        &self,
        _event: &EventData<ResponsePayload>,
        conversation: &mut Conversation,
    ) -> anyhow::Result<()> {
        let notes = conversation.metrics.notes.items.clone();
        let Some(context) = conversation.context.as_mut() else {
            return Ok(());
        };
        let out_of_view = notes
            .iter()
            .any(|note| !context.messages.iter().any(|entry| mentions(entry, &note.label())));
        if out_of_view {
            let content = Element::new("system_reminder").text(reminder(&notes));
            context.messages.push(ContextMessage::user(content, None).into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use forge_domain::{
        Agent, ChatCompletionMessageFull, Context, Metrics, ModelId, Notes, ToolCallId, ToolName,
        ToolResult,
    };
    use pretty_assertions::assert_eq;

    use super::*;

    fn fixture_conversation(notes: &[&str], messages: Vec<ContextMessage>) -> Conversation {
        let mut scratchpad = Notes::default();
        for note in notes {
            scratchpad.add(note).unwrap();
        }
        let mut conversation = Conversation::generate();
        conversation.metrics = Metrics::default();
        conversation.metrics.notes = scratchpad;
        conversation.context =
            Some(messages.into_iter().fold(Context::default(), |ctx, message| ctx.add_message(message)));
        conversation
    }

    fn fixture_event() -> EventData<ResponsePayload> {
        EventData::new(
            Agent::new("forge", "test-provider".to_string().into(), ModelId::new("m")),
            ModelId::new("m"),
            ResponsePayload::new(ChatCompletionMessageFull {
                content: String::new(),
                thought_signature: None,
                reasoning: None,
                reasoning_details: None,
                tool_calls: vec![],
                usage: Default::default(),
                finish_reason: None,
                phase: None,
            }),
        )
    }

    fn saved(label: &str) -> ContextMessage {
        ContextMessage::Tool(
            ToolResult::new(ToolName::new("write_note"))
                .call_id(ToolCallId::new("c"))
                .success(format!("Saved as {label} (1 of at most 20 notes kept).")),
        )
    }

    async fn reminders_after(mut conversation: Conversation) -> Vec<String> {
        NotesHandler::new().handle(&fixture_event(), &mut conversation).await.unwrap();
        conversation
            .context
            .unwrap()
            .messages
            .iter()
            .filter_map(|entry| entry.message.content().filter(|text| text.contains("SCRATCHPAD")).map(str::to_string))
            .collect()
    }

    #[tokio::test]
    async fn test_a_note_whose_result_is_still_in_view_is_not_repeated() {
        let fixture = fixture_conversation(&["tests run with python3 -m unittest"], vec![saved("[note 1]")]);

        let actual = reminders_after(fixture).await;

        assert_eq!(actual, Vec::<String>::new());
    }

    #[tokio::test]
    async fn test_notes_a_summary_removed_come_back_verbatim_once() {
        let fixture = fixture_conversation(
            &["root cause: off-by-one in ledger.balance", "tests run with python3 -m unittest"],
            vec![ContextMessage::user("summary of earlier work", None), saved("[note 2]")],
        );

        let actual = reminders_after(fixture).await;

        let expected = vec![Element::new("system_reminder")
            .text(format!(
                "{HEADER}\n[note 1] root cause: off-by-one in ledger.balance\n[note 2] tests run with python3 -m unittest"
            ))
            .render()];
        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn test_the_reminder_itself_keeps_the_notes_in_view() {
        let mut fixture = fixture_conversation(&["keep me"], vec![ContextMessage::user("summary", None)]);
        NotesHandler::new().handle(&fixture_event(), &mut fixture).await.unwrap();

        let actual = reminders_after(fixture).await;

        assert_eq!(actual.len(), 1);
    }
}
