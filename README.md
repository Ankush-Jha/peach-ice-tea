<p align="center">
  <img src="documentation/assets/logo.svg" width="120" alt="Peach Ice Tea logo">
</p>

<h1 align="center">Peach Ice Tea</h1>

<p align="center">
  A coding-agent harness for the LCC × DevClub AI Coding Harness Hackathon:<br>
  one frozen prompt, one unattended run, and an evidence bundle judges can check without taking our word for anything.
</p>

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
| Local web UI | `make ui`: run a task, watch it live, browse evidence and A/B reports from a browser; loopback-only | `harness/ui/` (D-092) |

## How one run works

```mermaid
flowchart TD
    P["Frozen prompt"] --> M["Model call"]
    M --> T{"Tool calls?"}
    T -->|"yes"| X["Execute tools<br>read · write · patch · shell · fetch · search · todo · skill"]
    X --> M
    T -->|"no — stopping"| VG["Runtime verification gate:<br>harness runs the real tests itself"]
    VG -->|"tests fail"| M
    VG -->|"tests pass, or gate off"| EX["Exit"]
    M -.->|"context past threshold"| CP0

    subgraph CP["Compaction — cheapest stage first, lossless before lossy"]
        CP0["S0 · supersede stale results"] --> CP1["S1 · offload large results to handles"]
        CP1 --> CP2["S2 · drop by relevance score"]
        CP2 --> CP3["S3 · lossy summary — last resort"]
    end
    CP3 -.-> M

    EX --> EV["Evidence bundle<br>prompt · transcript · telemetry.jsonl · integrity.json · diff.patch · tests.json · exec.json"]
    EV --> RP["report.json / report.md"]
```

A test-integrity guard runs alongside every step: it captures a manifest of protected test files before the
run starts, refuses any edit to them at tool or shell dispatch, and verifies and restores them after — including
after the harness's own final test run, since a suite can write files of its own.

## Setup

Requirements: Rust (pinned by `rust-toolchain.toml`; `make setup` installs it with rustup if missing), `protoc`
(D-009; installed with Homebrew if missing, otherwise `apt-get install protobuf-compiler`), Node 22+.

```bash
git clone <this repository> && cd <it>
export AI_API_KEY=...        # the only credential; environment only, never written anywhere
make setup                   # release build → target/release/forge, npm deps, layout check
```

## Running a task

The evaluator interface is the root `Makefile` (MAKEFILE_EVAL.md, D-070):

```mermaid
flowchart LR
    A["git clone"] --> B["export AI_API_KEY"]
    B --> C["make setup"]
    C --> D["make run"]
    D --> F["one unattended run"]
    F --> G["evidence bundle +<br>report.json / report.md"]
    G --> H["make test (optional)"]
```

`make run` takes the issue from stdin
(piped, or typed and ended with Ctrl-D) or from `PROMPT=`, and runs it once, unattended, in the repository named by
`REPO` (by default the directory `make` was invoked from; it refuses to work on the harness itself):

```bash
make run REPO=/path/to/repository < issue.md
make run REPO=/path/to/repository PROMPT="Fix the failing test in stats.py"
cd /path/to/repository && make -f /path/to/harness/Makefile run    # REPO defaults to here
```

A model the Organising Committee prescribes is set with `make run MODEL=<model id>` or `PROFILE=<name>` — a
variable, never a source change (MAKEFILE_EVAL.md §4). `AI_API_KEY` is then handed to that profile as the provider
variable it expects. Without either set, the key's own shape picks a profile as a convenience (D-081): an AI Studio
key (`AIza…`) → `gemini`, an OpenRouter key (`sk-or-…`) → `openrouter`, anything else → `gemini` as the fallback.
Optional: `TEST_COMMAND=` (otherwise detected), `EVIDENCE_DIR=` (default `evidence/<UTC time>/`),
`MAX_DURATION_SECS=` (default 1800). `make test` runs the local hackathon suite live; `make check` runs the
offline checks (no key, no model); `make clean` removes build output and evidence.

Underneath, `make run` calls the entry point `harness/peach-ice-tea`, which can also be used directly from the
repository to work on:

```bash
OPENROUTER_API_KEY=... harness/peach-ice-tea --profile openrouter --evidence-dir /path/outside/the/repo \
  --test-command "python3 -m unittest discover -s tests -t . -v" --prompt-file issue.md
```

The last stdout line is the JSON outcome. Exit codes: 0 completed, 1 error, 2 tool-failure limit, 3 request
limit, 4 time budget, 5 interrupted, 6 doom-loop escalation (flagged, D-082). `forge report <evidence-dir>` regenerates the report offline.

**`make ui`** starts a small local web UI (D-092) as an alternative to the command line: start a task, watch
it run live against the same `harness/run-task` entry point `make run` uses, and browse past evidence bundles
and benchmark reports. Loopback-only; provider keys never reach the browser.

## Evaluating locally

```bash
npm run hackathon -- --agent reference             # fixtures solve; runner check is green
npm run hackathon -- --agent forge-cheat           # proves forge's own integrity guard (no model, no spend)
OPENROUTER_API_KEY=... npm run hackathon -- --agent forge --profile openrouter --max-requests 60 --max-duration-secs 1500
```

`cargo insta test --workspace` runs the full suite (≈2,940 tests), including end-to-end tests of the real
binary against a scripted model.

## Layout (HACKATHON §32)

| Path | Contents |
|---|---|
| `Makefile` | Evaluator interface: `setup`, `run`, `test`, `check`, `clean` (D-070) |
| `harness/` | Entry point (`peach-ice-tea`), `run-task` (what `make run` calls), the local web UI (`ui/`), build and layout scripts |
| `crates/` | The harness itself (Rust workspace; stays here for upstream merges, D-021) |
| `telemetry/` | Where the organizers' telemetry files will be vendored; describes our internal stream |
| `reporting/` | Where the organizers' reporting files will be vendored; describes our report |
| `configuration/` | Prompt template and provider profiles (keys never stored) |
| `documentation/` | Architecture (§25 answers) and upstream's README |
| `benchmarks/hackathon/` | Local hackathon-shaped suite and runner |
| `docs/harness/` | Spec, research, plan, task list and the decision log |

## Configuration

Provider profiles and feature flags are listed in [`configuration/README.md`](configuration/README.md). The model
is never fixed in source: whatever the Organising Committee prescribes at evaluation time is set with
`MODEL=<model id>` or `PROFILE=<name>` (MAKEFILE_EVAL.md §4) — both read from the environment, no code change
either way (D-049). Without either set, `make run` infers a profile from `AI_API_KEY`'s own shape as a convenience
(D-081): an AI Studio key (`AIza…`) → `gemini`, an OpenRouter key (`sk-or-…`) → `openrouter`, anything else →
`gemini` as the fallback. `.env.example` shows the one variable the harness reads.

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
