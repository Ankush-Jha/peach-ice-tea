-- Append-only event log per conversation (R-CTX-1, T3.1). conversations.context stays the
-- working view; this is the history compaction cannot destroy.
CREATE TABLE IF NOT EXISTS thread_events (
    conversation_id TEXT NOT NULL,
    seq BIGINT NOT NULL,
    turn_id TEXT,
    kind TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    artifact_hash TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (conversation_id, seq)
);
CREATE INDEX IF NOT EXISTS idx_thread_events_artifact_hash ON thread_events(artifact_hash);

-- Content-addressed store for large payloads, deduplicated by SHA-256.
CREATE TABLE IF NOT EXISTS artifacts (
    hash TEXT PRIMARY KEY NOT NULL,
    bytes BLOB NOT NULL,
    mime TEXT NOT NULL,
    size BIGINT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);
