// @generated automatically by Diesel CLI.

diesel::table! {
    conversations (conversation_id) {
        conversation_id -> Text,
        title -> Nullable<Text>,
        workspace_id -> BigInt,
        context -> Nullable<Text>,
        created_at -> Timestamp,
        updated_at -> Nullable<Timestamp>,
        metrics -> Nullable<Text>,
    }
}

diesel::table! {
    thread_events (conversation_id, seq) {
        conversation_id -> Text,
        seq -> BigInt,
        turn_id -> Nullable<Text>,
        kind -> Text,
        payload_json -> Text,
        artifact_hash -> Nullable<Text>,
        created_at -> Timestamp,
    }
}

diesel::table! {
    artifacts (hash) {
        hash -> Text,
        bytes -> Binary,
        mime -> Text,
        size -> BigInt,
        created_at -> Timestamp,
    }
}

diesel::allow_tables_to_appear_in_same_query!(conversations, thread_events, artifacts);
