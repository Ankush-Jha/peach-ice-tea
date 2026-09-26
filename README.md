# Peach Ice Tea

A coding-agent harness for the LCC × DevClub AI Coding Harness Hackathon: one frozen prompt, one unattended
run, and an evidence bundle judges can check without taking our word for anything.

**Provenance:** Peach Ice Tea is built on a fork of [ForgeCode](https://github.com/tailcallhq/forgecode)
(Rust), forked from upstream `304bf3b` (D-008). ForgeCode's own README is kept at
[`documentation/UPSTREAM_README.md`](documentation/UPSTREAM_README.md). The crates keep their `forge_*`
names so upstream merges stay possible (D-004). What we built, and why, is in
[`documentation/ARCHITECTURE.md`](documentation/ARCHITECTURE.md) and `docs/harness/DECISIONS.md`.

## What the harness adds

| Area | What it does | Where |
|---|---|---|
| Test integrity | Refuses edits to protected tests at dispatch (tools and shell), verifies them after the run, and restores anything changed | `crates/forge_harness/src/integrity/` |
| One-shot autonomy | Never waits for a person; wall-clock budget; SIGTERM/SIGINT still write the evidence | `crates/forge_main/src/harness_exec.rs` |
| Verified completion | Test detection and classification, a gate against stopping on unverified edits, recovery hints, the harness's own final test run | `crates/forge_harness/src/verify/` |
| Telemetry | JSONL events with provider-reported tokens, retries (with billed usage), tool/model correlation, compaction, tests, integrity; redacted before disk | `crates/forge_harness/src/telemetry/`, `crates/forge_app/src/hooks/telemetry.rs` |
| Evidence + report | Prompt, transcript, telemetry, integrity, diff, tests, `exec.json`, `report.json`/`report.md` and a checksum manifest on every exit path | `crates/forge_harness/src/{evidence,report}.rs` |
| Provider robustness | Gemini thinking level, DeepSeek cache accounting, fail-fast on quotas that cannot recover | `crates/forge_repo/src/provider/`, `crates/forge_domain/src/provider_quota.rs` |

## Setup

Requirements: Rust (the version is pinned by `rust-toolchain.toml`), `protoc` (D-009), Node 20+ for the eval suite.

```bash
harness/build.sh                     # release build → target/release/forge
export GEMINI_API_KEY=...            # environment only; never commit it
```

## Running a task

```bash
harness/peach-ice-tea --evidence-dir /path/outside/the/repo/evidence \
  --test-command "python3 -m unittest discover -s tests -t . -v" \
  --prompt-file prompt.md
```

It runs in the current directory (the repository to work on) with the evaluation profile
(`configuration/profiles/gemini`). The last stdout line is the JSON outcome. Exit codes: 0 completed,
1 error, 2 tool-failure limit, 3 request limit, 4 time budget, 5 interrupted. `forge report <evidence-dir>`
regenerates the report offline.

## Evaluating locally

```bash
npm run hackathon -- --agent reference             # fixtures solve; runner check is green
npm run hackathon -- --agent forge-cheat           # proves forge's own integrity guard (no model, no spend)
GEMINI_API_KEY=... npm run hackathon -- --agent forge --profile gemini --max-requests 40 --max-duration-secs 900
```

`cargo insta test --workspace` runs the full suite (≈2,940 tests), including end-to-end tests of the real
binary against a scripted model.

## Layout (HACKATHON §32)

| Path | Contents |
|---|---|
| `harness/` | Entry point (`peach-ice-tea`) and build script |
| `crates/` | The harness itself (Rust workspace; stays here for upstream merges, D-021) |
| `telemetry/` | Where the organizers' telemetry files will be vendored; describes our internal stream |
| `reporting/` | Where the organizers' reporting files will be vendored; describes our report |
| `configuration/` | Prompt template and provider profiles (keys never stored) |
| `documentation/` | Architecture (§25 answers) and upstream's README |
| `benchmarks/hackathon/` | Local hackathon-shaped suite and runner |
| `docs/harness/` | Spec, research, plan, task list and the decision log |

## Configuration

Provider profiles and feature flags are listed in [`configuration/README.md`](configuration/README.md).
The judged profile is always `gemini`. Two DeepSeek profiles exist for development only (D-036, D-043).

## Major design decisions

Enforce in the runtime, not in the prompt (D-019, D-031, D-037). Write evidence on every exit path
(D-034, D-038). Objective numbers only, with discrepancies shown rather than reconciled (D-035). Behaviour
changes stay behind default-off flags until an A/B supports them (D-039, D-041, D-042). The full log is
`docs/harness/DECISIONS.md`.

## Known limitations

- The organizers' telemetry and report schemas are not published yet; the adapters are stubs (D-020).
- Edits made through shell commands don't arm the verify gate; only tool edits do (D-037).
- The title-generation model call is not metered.
- SIGKILL cannot be caught. A run killed that way keeps only what was written as it went: the prompt, streamed
  telemetry, a provisional `manifest.json` reading `incomplete`, and `integrity.baseline.json` with each test's
  pre-run hash and the location of its copy. There is no transcript, report or restore (D-038, D-056).
- No A/B has been affordable yet (D-025), so the flagged features are unmeasured.
- Live-model evidence so far is limited (D-032, D-040, D-043).
