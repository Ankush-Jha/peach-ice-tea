//! Storage for conversation event logs and their artifacts (R-CTX-1, T3.1).

mod writer;

use std::sync::Arc;

use diesel::prelude::*;
use peach_domain::{ConversationId, StoredThreadEvent, ThreadEvent, ThreadEventRepository};
use sha2::{Digest, Sha256};
pub use writer::EventLogWriter;

use crate::database::schema::{artifacts, thread_events};
use crate::database::{DatabasePool, PooledSqliteConnection};

/// Payloads at least this large (serialised) move to the artifact store and
/// the event row keeps only a reference.
pub const DEFAULT_ARTIFACT_MIN_BYTES: usize = 16 * 1024;

/// The artifact store never grows past this. When an artifact would exceed
/// it, the payload stays inline in its event row instead: nothing is lost,
/// only deduplication.
pub const DEFAULT_ARTIFACT_CAP_BYTES: u64 = 512 * 1024 * 1024;

const EVENT_MIME: &str = "application/vnd.peach.thread-event+json";

#[derive(Insertable)]
#[diesel(table_name = thread_events)]
struct NewEventRow {
    conversation_id: String,
    seq: i64,
    turn_id: Option<String>,
    kind: String,
    payload_json: String,
    artifact_hash: Option<String>,
    created_at: chrono::NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = artifacts)]
struct NewArtifactRow {
    hash: String,
    bytes: Vec<u8>,
    mime: String,
    size: i64,
    created_at: chrono::NaiveDateTime,
}

/// SQLite-backed [`ThreadEventRepository`].
pub struct ThreadEventRepositoryImpl {
    pool: Arc<DatabasePool>,
    artifact_min_bytes: usize,
    artifact_cap_bytes: u64,
}

impl ThreadEventRepositoryImpl {
    /// Creates the repository with the default artifact threshold and cap.
    pub fn new(pool: Arc<DatabasePool>) -> Self {
        Self {
            pool,
            artifact_min_bytes: DEFAULT_ARTIFACT_MIN_BYTES,
            artifact_cap_bytes: DEFAULT_ARTIFACT_CAP_BYTES,
        }
    }

    /// Overrides the artifact threshold and store cap.
    pub fn artifact_limits(mut self, min_bytes: usize, cap_bytes: u64) -> Self {
        self.artifact_min_bytes = min_bytes;
        self.artifact_cap_bytes = cap_bytes;
        self
    }

    async fn run<F, T>(&self, operation: F) -> anyhow::Result<T>
    where
        F: FnOnce(&mut PooledSqliteConnection) -> anyhow::Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = pool.get_connection()?;
            operation(&mut connection)
        })
        .await
        .map_err(|e| anyhow::anyhow!("Thread event repository task failed: {e}"))?
    }

    /// Deletes artifacts that no event of a live conversation references
    /// (a conversation is live while its row exists). Returns how many were
    /// deleted.
    ///
    /// # Errors
    /// Returns an error if storage fails.
    pub async fn gc_artifacts(&self) -> anyhow::Result<usize> {
        self.run(|connection| {
            let deleted = diesel::sql_query(
                "DELETE FROM artifacts WHERE hash NOT IN (
                    SELECT e.artifact_hash FROM thread_events e
                    JOIN conversations c ON c.conversation_id = e.conversation_id
                    WHERE e.artifact_hash IS NOT NULL
                )",
            )
            .execute(connection)?;
            Ok(deleted)
        })
        .await
    }
}

/// Holds the row type for the artifact-store size query. Its own module so
/// the lint allowance below covers only the code diesel's derive generates
/// (`total: total`, flagged by nightly clippy), and nothing written by hand.
mod store_size {
    #![allow(clippy::redundant_field_names)]
    use diesel::QueryableByName;

    #[derive(QueryableByName)]
    pub(super) struct StoreSize {
        #[diesel(sql_type = diesel::sql_types::BigInt)]
        pub(super) total: i64,
    }
}
use store_size::StoreSize;

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Stores `payload` as an artifact if it fits under the cap; returns its hash,
/// or `None` to keep it inline.
fn store_artifact(
    connection: &mut SqliteConnection,
    payload: &str,
    cap_bytes: u64,
) -> anyhow::Result<Option<String>> {
    let hash = sha256_hex(payload.as_bytes());
    let exists = artifacts::table
        .find(&hash)
        .select(artifacts::hash)
        .first::<String>(connection)
        .optional()?
        .is_some();
    if exists {
        return Ok(Some(hash));
    }
    let used = diesel::sql_query("SELECT COALESCE(SUM(size), 0) AS total FROM artifacts")
        .get_result::<StoreSize>(connection)?
        .total;
    if used as u64 + payload.len() as u64 > cap_bytes {
        tracing::warn!(
            size = payload.len(),
            cap_bytes,
            "Artifact store full; keeping the payload inline"
        );
        return Ok(None);
    }
    diesel::insert_into(artifacts::table)
        .values(NewArtifactRow {
            hash: hash.clone(),
            bytes: payload.as_bytes().to_vec(),
            mime: EVENT_MIME.to_string(),
            size: payload.len() as i64,
            created_at: chrono::Utc::now().naive_utc(),
        })
        .execute(connection)?;
    Ok(Some(hash))
}

#[async_trait::async_trait]
impl ThreadEventRepository for ThreadEventRepositoryImpl {
    async fn append_events(
        &self,
        conversation_id: &ConversationId,
        events: Vec<ThreadEvent>,
    ) -> anyhow::Result<()> {
        if events.is_empty() {
            return Ok(());
        }
        let conversation_id = conversation_id.into_string();
        let (min_bytes, cap_bytes) = (self.artifact_min_bytes, self.artifact_cap_bytes);
        self.run(move |connection| {
            connection.transaction::<_, anyhow::Error, _>(|connection| {
                let last: Option<i64> = thread_events::table
                    .filter(thread_events::conversation_id.eq(&conversation_id))
                    .select(diesel::dsl::max(thread_events::seq))
                    .first(connection)?;
                for (seq, event) in (last.unwrap_or(0) + 1..).zip(events) {
                    let payload = serde_json::to_string(&event)?;
                    let artifact_hash = if payload.len() >= min_bytes {
                        store_artifact(connection, &payload, cap_bytes)?
                    } else {
                        None
                    };
                    let payload_json = match &artifact_hash {
                        Some(hash) => serde_json::json!({ "artifact": hash }).to_string(),
                        None => payload,
                    };
                    diesel::insert_into(thread_events::table)
                        .values(NewEventRow {
                            conversation_id: conversation_id.clone(),
                            seq,
                            turn_id: None,
                            kind: event.kind().to_string(),
                            payload_json,
                            artifact_hash,
                            created_at: chrono::Utc::now().naive_utc(),
                        })
                        .execute(connection)?;
                }
                Ok(())
            })
        })
        .await
    }

    async fn list_events(
        &self,
        conversation_id: &ConversationId,
    ) -> anyhow::Result<Vec<StoredThreadEvent>> {
        let id = *conversation_id;
        self.run(move |connection| {
            let rows: Vec<(i64, String, Option<String>)> = thread_events::table
                .filter(thread_events::conversation_id.eq(id.into_string()))
                .order(thread_events::seq.asc())
                .select((
                    thread_events::seq,
                    thread_events::payload_json,
                    thread_events::artifact_hash,
                ))
                .load(connection)?;
            rows.into_iter()
                .map(|(seq, payload_json, artifact_hash)| {
                    let payload = match artifact_hash {
                        Some(hash) => {
                            let bytes: Vec<u8> = artifacts::table
                                .find(&hash)
                                .select(artifacts::bytes)
                                .first(connection)
                                .map_err(|e| {
                                    anyhow::anyhow!("event {seq}: artifact {hash} is missing: {e}")
                                })?;
                            String::from_utf8(bytes)?
                        }
                        None => payload_json,
                    };
                    let event: ThreadEvent = serde_json::from_str(&payload)
                        .map_err(|e| anyhow::anyhow!("event {seq} cannot be decoded: {e}"))?;
                    Ok(StoredThreadEvent { conversation_id: id, seq: seq as u64, event })
                })
                .collect()
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use peach_domain::{
        Context, ContextMessage, Conversation, ConversationRepository, ToolName, ToolResult,
        WorkspaceHash, events_between, replay_history, replay_view,
    };
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::conversation::ConversationRepositoryImpl;

    fn fixture() -> (Arc<DatabasePool>, ThreadEventRepositoryImpl) {
        let pool = Arc::new(DatabasePool::in_memory().unwrap());
        let repo = ThreadEventRepositoryImpl::new(pool.clone()).artifact_limits(1024, 1024 * 1024);
        (pool, repo)
    }

    fn fixture_context() -> Context {
        Context::default()
            .add_message(ContextMessage::user("Fix the adder.", None))
            .add_message(ContextMessage::tool_result(
                ToolResult::new(ToolName::new("read")).success("x".repeat(5_000)),
            ))
            .add_message(ContextMessage::assistant("Found it.", None, None, None))
    }

    #[tokio::test]
    async fn test_a_stored_log_replays_the_pre_compaction_context_byte_for_byte() {
        let (_pool, repo) = fixture();
        let id = ConversationId::generate();
        let before = fixture_context();
        let compacted = Context::default().add_message(ContextMessage::user("Summary.", None));
        repo.append_events(&id, events_between(&[], &before.messages))
            .await
            .unwrap();
        repo.append_events(&id, events_between(&before.messages, &compacted.messages))
            .await
            .unwrap();

        let stored = repo.list_events(&id).await.unwrap();
        let events: Vec<ThreadEvent> = stored.iter().map(|s| s.event.clone()).collect();
        let actual = (
            stored.iter().map(|s| s.seq).collect::<Vec<_>>(),
            serde_json::to_string(&replay_history(&events)).unwrap(),
            replay_view(&events),
        );

        let expected = (
            vec![1, 2, 3, 4],
            serde_json::to_string(&before.messages).unwrap(),
            compacted.messages,
        );
        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn test_large_payloads_are_stored_once_and_read_back() {
        let (pool, repo) = fixture();
        let id = ConversationId::generate();
        let big = fixture_context().messages[1].clone();
        let events = vec![
            ThreadEvent::Message { entry: Box::new(big.clone()) },
            ThreadEvent::Message { entry: Box::new(big.clone()) },
        ];

        repo.append_events(&id, events).await.unwrap();

        let artifact_count: i64 = artifacts::table
            .count()
            .get_result(&mut pool.get_connection().unwrap())
            .unwrap();
        let actual: Vec<ThreadEvent> = repo
            .list_events(&id)
            .await
            .unwrap()
            .into_iter()
            .map(|s| s.event)
            .collect();
        assert_eq!(artifact_count, 1, "identical payloads share one artifact");
        assert_eq!(
            actual,
            vec![
                ThreadEvent::Message { entry: Box::new(big.clone()) },
                ThreadEvent::Message { entry: Box::new(big) }
            ]
        );
    }

    #[tokio::test]
    async fn test_over_the_cap_the_payload_stays_inline_and_nothing_is_lost() {
        let pool = Arc::new(DatabasePool::in_memory().unwrap());
        let repo = ThreadEventRepositoryImpl::new(pool.clone()).artifact_limits(1024, 2048);
        let id = ConversationId::generate();
        let big = fixture_context().messages[1].clone();

        repo.append_events(
            &id,
            vec![ThreadEvent::Message { entry: Box::new(big.clone()) }],
        )
        .await
        .unwrap();

        let artifact_count: i64 = artifacts::table
            .count()
            .get_result(&mut pool.get_connection().unwrap())
            .unwrap();
        let actual: Vec<ThreadEvent> = repo
            .list_events(&id)
            .await
            .unwrap()
            .into_iter()
            .map(|s| s.event)
            .collect();
        assert_eq!(artifact_count, 0);
        assert_eq!(actual, vec![ThreadEvent::Message { entry: Box::new(big) }]);
    }

    #[tokio::test]
    async fn test_gc_keeps_artifacts_of_live_conversations_only() {
        let (pool, repo) = fixture();
        let conversations = ConversationRepositoryImpl::new(pool.clone(), WorkspaceHash::new(0));
        let live = Conversation::new(ConversationId::generate());
        conversations
            .upsert_conversation(live.clone())
            .await
            .unwrap();
        let gone = ConversationId::generate();
        let entry = |text: &str| ThreadEvent::Message {
            entry: Box::new(
                ContextMessage::tool_result(
                    ToolResult::new(ToolName::new("read")).success(text.repeat(2_000)),
                )
                .into(),
            ),
        };
        repo.append_events(&live.id, vec![entry("a")])
            .await
            .unwrap();
        repo.append_events(&gone, vec![entry("b")]).await.unwrap();

        let deleted = repo.gc_artifacts().await.unwrap();

        assert_eq!(deleted, 1);
        assert_eq!(repo.list_events(&live.id).await.unwrap().len(), 1);
        assert!(
            repo.list_events(&gone).await.is_err(),
            "the orphan's artifact is gone, and reading it says so"
        );
    }

    #[tokio::test]
    async fn test_a_conversation_from_before_the_log_has_no_events() {
        let (_pool, repo) = fixture();

        let actual = repo.list_events(&ConversationId::generate()).await.unwrap();

        assert_eq!(actual, vec![]);
    }
}
