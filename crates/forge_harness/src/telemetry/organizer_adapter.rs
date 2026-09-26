//! STUB — maps our internal telemetry envelope to the organizers'
//! `telemetry.schema.json`, once it is published.
//!
//! `HACKATHON.md` §14 says the organizers supply `telemetry/telemetry.md`,
//! `telemetry/telemetryAgent.md` and `telemetry/telemetry.schema.json`; §16
//! requires the vendored copies of those files to stay byte-for-byte
//! unchanged (protocol id, version, checksum may be verified). Because the
//! schema does not exist yet, this module cannot know the organizers' field
//! names, so it does not guess at them (`DECISIONS.md` D-020: "isolate the
//! mapping to the organizers' schemas in one adapter module per schema, so
//! the day they are published only the adapter changes").
//!
//! **When `telemetry.schema.json` is published:**
//! 1. Vendor it byte-for-byte into `telemetry/` (do not edit it) and add the
//!    checksum check referenced in §16.
//! 2. Fill in [`to_organizer`] (and, if the organizers batch events rather
//!    than stream them, [`to_organizer_batch`]) to build a
//!    [`serde_json::Value`] — or a generated type, if one is derived from
//!    the schema — matching their structure field-by-field.
//! 3. Nothing in `event.rs`, `sink.rs` or `mod.rs` should need to change:
//!    the internal [`Envelope`] is the only input this file reads, and nothing
//!    else reads this file's output until a later piece wires it up.

use crate::telemetry::event::Envelope;

/// Converts one internal [`Envelope`] to the organizers' telemetry schema.
///
/// STUB: always returns `None` because `telemetry.schema.json` has not been
/// published yet. Once it is, this becomes the single place that knows the
/// organizers' field names; every other module in this crate keeps using
/// the internal schema.
pub fn to_organizer(envelope: &Envelope) -> Option<serde_json::Value> {
    let _ = envelope;
    None
}

/// Converts a run's worth of internal envelopes to the organizers' schema.
///
/// STUB: always returns an empty vector, for the same reason as
/// [`to_organizer`]. Kept as a separate signature because the organizers'
/// protocol may expect one document per run rather than one per event; once
/// the schema is published, implement whichever shape it actually wants and
/// delete whichever of the two signatures turns out to be unneeded.
pub fn to_organizer_batch(envelopes: &[Envelope]) -> Vec<serde_json::Value> {
    envelopes.iter().filter_map(to_organizer).collect()
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::identity::HarnessIdentity;
    use crate::telemetry::event::{RunStart, TelemetryEvent};

    fn fixture_envelope() -> Envelope {
        Envelope {
            schema_id: "peach-ice-tea.telemetry".to_string(),
            schema_version: "0.1.0".to_string(),
            harness: HarnessIdentity::current(),
            run_id: "run-1".to_string(),
            seq: 0,
            timestamp: "2026-09-23T10:15:30.123Z".to_string(),
            conversation_id: None,
            agent_id: None,
            event: TelemetryEvent::RunStart(RunStart {
                task_id: None,
                repo_root: "/repo".to_string(),
            }),
        }
    }

    #[test]
    fn test_stub_produces_nothing_until_the_schema_is_published() {
        let fixture = fixture_envelope();

        assert_eq!(to_organizer(&fixture), None);
        assert_eq!(to_organizer_batch(&[fixture]), Vec::<serde_json::Value>::new());
    }
}
