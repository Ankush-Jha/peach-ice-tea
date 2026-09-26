# reporting/

**Organizer files** (`reportCreating.md`, `reportCreatorAgent.md`, `report.schema.json`, HACKATHON.md §17)
will be vendored here byte-for-byte when published. Until then, the harness generates its **internal**
standard report (HACKATHON §18) into every evidence bundle:

- `report.json` (schema version `REPORT_SCHEMA_VERSION`) and `report.md`, built by
  `crates/peach_harness/src/report.rs` from the bundle's `exec.json`, `telemetry.jsonl`, `integrity.json`,
  `tests.json` and `diff.patch`. Objective values only; a missing input reads "not available", never 0.
- Regenerate offline with `peach report <evidence-dir>`. It reads the bundle only and re-runs nothing.
