# telemetry/

**Organizer files** (`telemetry.md`, `telemetryAgent.md`, `telemetry.schema.json`, HACKATHON.md §14) will be
vendored here byte-for-byte when published and must never be edited (§16). Until then:

- The harness writes its **internal** event stream (`peach-ice-tea.telemetry`, schema version in
  `crates/forge_harness/src/identity.rs`) to `<evidence-dir>/telemetry.jsonl`: one JSON envelope per line,
  with `seq`, an RFC 3339 `timestamp`, `run_id`, `conversation_id` and `agent_id`, and an `event` tagged by
  `type`.
- Event types (`crates/forge_harness/src/telemetry/event.rs`): `run_start`, `run_end`, `agent_state`,
  `model_call`, `tool_call`, `retry`, `context_compaction`, `test_run`, `recovery`, `integrity`,
  `prompt_suppressed`. (`context_composition` and `error` are defined but not emitted yet.)
- Token counts come only from provider usage (§16). The one local estimate is named
  `context_tokens_estimated`.
- Mapping to the organizers' schema belongs in `crates/forge_harness/src/telemetry/organizer_adapter.rs`
  (a stub until the schema exists, D-020).
