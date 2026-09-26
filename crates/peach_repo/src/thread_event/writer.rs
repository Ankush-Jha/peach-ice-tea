//! Writes each saved conversation's changes to its event log (R-CTX-1, T3.2).

use std::collections::HashMap;
use std::sync::Arc;

use peach_domain::{
    Conversation, ConversationId, MessageEntry, ThreadEvent, ThreadEventRepository, events_between, replay_view,
};
use tokio::sync::Mutex;

/// Appends, on every save, the events that turn the conversation's logged
/// view into its current context: new messages as `Message` events, a
/// rewrite (compaction) as one `Compaction` event.
///
/// The comparison is against the log's own view, never the stored `context`
/// column, which round-trips through repository records and could differ in
/// ways that would look like a rewrite. The view is cached per conversation
/// and rebuilt from the log on a miss, so a resumed conversation continues
/// its log instead of starting a new one.
///
/// Scratchpad notes (R-CTX-10, D-087) are not in the context; each new one is
/// appended as a `Note` event, tracked by the highest note id logged.
pub struct EventLogWriter<R> {
    repository: Arc<R>,
    views: Mutex<HashMap<ConversationId, (Vec<MessageEntry>, u64)>>,
}

impl<R> EventLogWriter<R> {
    /// Creates a writer over `repository`.
    pub fn new(repository: Arc<R>) -> Self {
        Self { repository, views: Mutex::new(HashMap::new()) }
    }
}

impl<R: ThreadEventRepository> EventLogWriter<R> {
    /// Records `conversation`'s context changes since the last record. A
    /// conversation without a context has nothing to record.
    ///
    /// # Errors
    /// Returns an error if the log cannot be read or appended to; the cached
    /// view is left as it was, so the next record retries the same diff.
    pub async fn record(&self, conversation: &Conversation) -> anyhow::Result<()> {
        let Some(context) = &conversation.context else {
            return Ok(());
        };
        let mut views = self.views.lock().await;
        let (before, notes_logged) = match views.get(&conversation.id) {
            Some(logged) => logged.clone(),
            None => {
                let stored: Vec<ThreadEvent> =
                    self.repository.list_events(&conversation.id).await?.into_iter().map(|s| s.event).collect();
                let notes_logged = stored
                    .iter()
                    .filter_map(|event| match event {
                        ThreadEvent::Note { id, .. } => Some(*id),
                        _ => None,
                    })
                    .max()
                    .unwrap_or(0);
                (replay_view(&stored), notes_logged)
            }
        };
        let mut events = events_between(&before, &context.messages);
        let new_notes = conversation.metrics.notes.items.iter().filter(|note| note.id > notes_logged);
        events.extend(new_notes.map(|note| ThreadEvent::Note { id: note.id, text: note.text.clone() }));
        if !events.is_empty() {
            self.repository.append_events(&conversation.id, events).await?;
        }
        let notes_logged = conversation.metrics.notes.written.max(notes_logged);
        views.insert(conversation.id, (context.messages.clone(), notes_logged));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use peach_domain::{Context, ContextMessage, ThreadEvent, replay_history};
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::database::DatabasePool;
    use crate::thread_event::ThreadEventRepositoryImpl;

    fn fixture() -> (Arc<ThreadEventRepositoryImpl>, Conversation) {
        let pool = Arc::new(DatabasePool::in_memory().unwrap());
        (Arc::new(ThreadEventRepositoryImpl::new(pool)), Conversation::new(ConversationId::generate()))
    }

    fn with_messages(conversation: &Conversation, texts: &[&str]) -> Conversation {
        let context = texts.iter().fold(Context::default(), |ctx, text| ctx.add_message(ContextMessage::user(*text, None)));
        conversation.clone().context(context)
    }

    async fn kinds(repository: &ThreadEventRepositoryImpl, id: &ConversationId) -> Vec<&'static str> {
        repository.list_events(id).await.unwrap().iter().map(|s| s.event.kind()).collect()
    }

    #[tokio::test]
    async fn test_saves_become_messages_and_a_compaction_and_the_view_is_a_projection() {
        let (repository, conversation) = fixture();
        let writer = EventLogWriter::new(repository.clone());

        for texts in [vec!["a"], vec!["a", "b"], vec!["a", "b"], vec!["summary of a, b"], vec!["summary of a, b", "c"]] {
            writer.record(&with_messages(&conversation, &texts)).await.unwrap();
        }

        let events: Vec<ThreadEvent> =
            repository.list_events(&conversation.id).await.unwrap().into_iter().map(|s| s.event).collect();
        let actual = (
            kinds(&repository, &conversation.id).await,
            replay_view(&events),
            replay_history(&events).len(),
        );
        let expected = (
            vec!["message", "message", "compaction", "message"],
            with_messages(&conversation, &["summary of a, b", "c"]).context.unwrap().messages,
            3,
        );
        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn test_a_resumed_conversation_continues_its_log_without_a_spurious_compaction() {
        let (repository, conversation) = fixture();
        EventLogWriter::new(repository.clone()).record(&with_messages(&conversation, &["a", "b"])).await.unwrap();

        // A new process: nothing cached.
        EventLogWriter::new(repository.clone()).record(&with_messages(&conversation, &["a", "b", "c"])).await.unwrap();

        assert_eq!(kinds(&repository, &conversation.id).await, vec!["message", "message", "message"]);
    }

    #[tokio::test]
    async fn test_a_conversation_from_before_the_log_is_seeded_on_its_first_save() {
        let (repository, conversation) = fixture();

        EventLogWriter::new(repository.clone()).record(&with_messages(&conversation, &["old 1", "old 2"])).await.unwrap();

        assert_eq!(kinds(&repository, &conversation.id).await, vec!["message", "message"]);
    }

    #[tokio::test]
    async fn test_each_note_is_logged_once_even_across_a_resume() {
        let (repository, conversation) = fixture();
        let mut noted = with_messages(&conversation, &["a"]);
        noted.metrics.notes.add("root cause: off-by-one").unwrap();
        EventLogWriter::new(repository.clone()).record(&noted).await.unwrap();
        noted.metrics.notes.add("tests: python3 -m unittest").unwrap();

        // A new process, which must not log note 1 again.
        EventLogWriter::new(repository.clone()).record(&noted).await.unwrap();

        let actual: Vec<ThreadEvent> =
            repository.list_events(&conversation.id).await.unwrap().into_iter().map(|s| s.event).skip(1).collect();
        let expected = vec![
            ThreadEvent::Note { id: 1, text: "root cause: off-by-one".to_string() },
            ThreadEvent::Note { id: 2, text: "tests: python3 -m unittest".to_string() },
        ];
        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn test_no_context_records_nothing() {
        let (repository, conversation) = fixture();

        EventLogWriter::new(repository.clone()).record(&conversation).await.unwrap();

        assert_eq!(kinds(&repository, &conversation.id).await, Vec::<&str>::new());
    }
}
