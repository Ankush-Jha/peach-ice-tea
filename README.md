# Peach Ice Tea

A coding-agent harness for the LCC × DevClub AI Coding Harness Hackathon: one frozen prompt, one unattended
run, and an evidence bundle judges can check without taking our word for anything.

**Provenance:** Peach Ice Tea is built on a fork of [Peach Ice Tea](https://github.com/Ankush-Jha/peach-ice-tea)
(Rust), forked from upstream `304bf3b` (D-008). Peach Ice Tea's own README is kept at
[`documentation/UPSTREAM_README.md`](documentation/UPSTREAM_README.md). The crates keep their `peach_*`
names so upstream merges stay possible (D-004). What we built, and why, is in
[`documentation/ARCHITECTURE.md`](documentation/ARCHITECTURE.md) and `docs/harness/DECISIONS.md`.

## What the harness adds

| Area | What it does | Where |
|---|---|---|
| Test integrity | Refuses edits to protected tests at dispatch (tools and shell), verifies them after the run, and restores anything changed | `crates/peach_harness/src/integrity/` |
| One-shot autonomy | Never waits for a person; wall-clock budget; SIGTERM/SIGINT still write the evidence | `crates/peach_main/src/harness_exec.rs` |
| Verified completion | Test detection and classification, a gate against stopping on unverified edits, recovery hints, the harness's own final test run | `crates/peach_harness/src/verify/` |
| Telemetry | JSONL events with provider-reported tokens, retries (with billed usage), tool/model correlation, compaction, tests, integrity; redacted before disk | `crates/peach_harness/src/telemetry/`, `crates/peach_app/src/hooks/telemetry.rs` |
| Evidence + report | Prompt, transcript, telemetry, integrity, diff, tests, `exec.json`, `report.json`/`report.md` and a checksum manifest on every exit path | `crates/peach_harness/src/{evidence,report}.rs` |
| Provider robustness | Gemini thinking level, DeepSeek cache accounting, fail-fast on quotas that cannot recover | `crates/peach_repo/src/provider/`, `crates/peach_domain/src/provider_quota.rs` |

## Setup

Requirements: Rust (pinned by `rust-toolchain.toml`; `make setup` installs it with rustup if missing), `protoc`
(D-009; installed with Homebrew if missing, otherwise `apt-get install protobuf-compiler`), Node 22+.

```bash
git clone <this repository> && cd <it>
export AI_API_KEY=...        # the only credential; environment only, never written anywhere
make setup                   # release build → target/release/peach, npm deps, layout check
```

## Running a task

The evaluator interface is the root `Makefile` (MAKEFILE_EVAL.md, D-070). `make run` takes the issue from stdin
(piped, or typed and ended with Ctrl-D) or from `PROMPT=`, and runs it once, unattended, in the repository named by
`REPO` (by default the directory `make` was invoked from; it refuses to work on the harness itself):

```bash
make run REPO=/path/to/repository < issue.md
make run REPO=/path/to/repository PROMPT="Fix the failing test in stats.py"
cd /path/to/repository && make -f /path/to/harness/Makefile run    # REPO defaults to here
```

`AI_API_KEY` is handed to the selected profile as the provider variable it expects, so a prescribed model is a
variable, not a source change: `make run PROFILE=openrouter MODEL=<model id>`, or `PROFILE=gemini` for Google
AI Studio. Optional: `TEST_COMMAND=` (otherwise detected), `EVIDENCE_DIR=` (default `evidence/<UTC time>/`),
`MAX_DURATION_SECS=` (default 1800). `make test` runs the local hackathon suite live; `make check` runs the
offline checks (no key, no model); `make clean` removes build output and evidence.

Underneath, `make run` calls the entry point `harness/peach-ice-tea`, which can also be used directly from the
repository to work on:

```bash
OPENROUTER_API_KEY=... harness/peach-ice-tea --profile openrouter --evidence-dir /path/outside/the/repo \
  --test-command "python3 -m unittest discover -s tests -t . -v" --prompt-file issue.md
```

The last stdout line is the JSON outcome. Exit codes: 0 completed, 1 error, 2 tool-failure limit, 3 request
limit, 4 time budget, 5 interrupted. `peach report <evidence-dir>` regenerates the report offline.

## Evaluating locally

```bash
npm run hackathon -- --agent reference             # fixtures solve; runner check is green
npm run hackathon -- --agent peach-cheat           # proves peach's own integrity guard (no model, no spend)
OPENROUTER_API_KEY=... npm run hackathon -- --agent peach --profile openrouter --max-requests 60 --max-duration-secs 1500
```

`cargo insta test --workspace` runs the full suite (≈2,940 tests), including end-to-end tests of the real
binary against a scripted model.

## Layout (HACKATHON §32)

| Path | Contents |
|---|---|
| `Makefile` | Evaluator interface: `setup`, `run`, `test`, `check`, `clean` (D-070) |
| `harness/` | Entry point (`peach-ice-tea`), `run-task` (what `make run` calls), build and layout scripts |
| `crates/` | The harness itself (Rust workspace; stays here for upstream merges, D-021) |
| `telemetry/` | Where the organizers' telemetry files will be vendored; describes our internal stream |
| `reporting/` | Where the organizers' reporting files will be vendored; describes our report |
| `configuration/` | Prompt template and provider profiles (keys never stored) |
| `documentation/` | Architecture (§25 answers) and upstream's README |
| `benchmarks/hackathon/` | Local hackathon-shaped suite and runner |
| `docs/harness/` | Spec, research, plan, task list and the decision log |

## Configuration

Provider profiles and feature flags are listed in [`configuration/README.md`](configuration/README.md).
Any model may be used (D-049). The default is `openrouter` on a free-tier model (D-051, D-069); `.env.example`
shows the one variable the harness reads.

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
- No A/B has run on two model families yet (free tier only, D-069), so the flagged features stay off.
- Live-model evidence so far is limited (D-032, D-040, D-043, D-051).
- `make run` is one-shot: it takes one issue and exits. If the organisers want a harness that stays resident and
  takes several issues in one session, that is new scope (MAKEFILE_EVAL.md).
