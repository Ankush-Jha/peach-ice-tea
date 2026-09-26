# Alignment with the hackathon specification — Peach Ice Tea

**Peach Ice Tea** is our harness (D-023): built on a ForgeCode fork, entered in the LCC × DevClub AI Coding Harness
Hackathon.

`HACKATHON.md` is the governing document. `SPEC.md`/`TASKS.md` were written for a general "best-in-class harness"
and assumed an A/B across two model families (Anthropic + OpenAI). The hackathon fixes the model (**Gemini 3.8
High**), judges **one frozen, unattended run** on a hidden repository and issue, and scores it from the outcome,
transcript, telemetry, a standard report, and a technical interview. This file maps one onto the other and
re-prioritises the backlog. Decisions are recorded as D-017 … D-022 in `DECISIONS.md`.

## 1. Rubric → what earns the points

| Rubric (weight) | What the judges look at | What we have | Gap → requirement |
|---|---|---|---|
| **Correctness (30)** | Tests pass, issue solved, repo state | Forge's loop, `patch`, verification + todo enforcement | Must never hang or stall in one-shot → **R-HACK-1**; verified completion gate → **R-HACK-7**; never touch tests → **R-HACK-2**; Gemini-specific reliability → **R-HACK-6** |
| **Orchestration (15)** | Loop, planning, state, termination, adaptation | Orchestrator, todos, doom-loop detection, subagents | Explicit, observable agent states and termination reasons in telemetry → **R-HACK-3**; parallel read-only batches (T2.1) |
| **Context mgmt (15)** | Relevance, growth, compression, retention | Compaction (lossy, overwrites history) | M1 output shaping (T1.1–T1.5), event log + reversible compaction (T3.1–T3.6), context size per call in telemetry |
| **Tools (10)** | Design, selection, redundancy, failure recovery | Mature toolset; schema rules | Loud truncation (T1.1), recovery events (T1.3), pre-dispatch correction (T2.6), per-tool error rates in telemetry |
| **Token efficiency (10)** | Useful work per token; repeated context | `TaskMetrics` (T0.2) | Per-call tokens and context size, cache hit rate (Gemini implicit cache), line numbers off (T1.2), noise compression (T1.5) |
| **Error recovery (10)** | Failed commands/tests/tools, regressions, unusable actions | Retries, tool-failure limit, doom loop | Failure classification (build vs test vs env vs tool) + recovery actions recorded → **R-HACK-7**, **R-HACK-3** |
| **Code quality (5)** | Fits architecture, minimal change | Prompts | Minimal-diff guidance, diff stats in the report → **R-HACK-4** |
| **Technical understanding (5)** | Interview | `DECISIONS.md`, `RECON.md` | Architecture document grounded in real telemetry → **R-HACK-9** |

## 2. New requirements

### R-HACK-1 — One-shot autonomy (never block, never ask)
`forge exec` in the evaluation profile must complete or fail on its own. No interactive prompt may be reachable: the
"continue anyway?" confirm (fixed in T0.3), the `followup` tool, permission/approval prompts, model/provider pickers,
login flows, update prompts, and any `ForgeWidget` reachable from `on_message`. Missing configuration fails fast with
outcome `error`. Wall-clock and request budgets are configurable. **Acceptance:** with stdin closed and no TTY, every
such path either proceeds or exits non-zero within its budget; a test per path.

### R-HACK-2 — Test-integrity guard (enforced in the runtime)
Editing a protected test is disqualifying (HACKATHON §8, §31), so this is enforced, not prompted (CLAUDE.md
principle 3). Before the run: resolve protected paths (configurable globs; defaults cover `tests/`, `test/`,
`__tests__/`, `spec/`, `*_test.*`, `test_*.*`, `*.test.*`, `*.spec.*`, `conftest.py`, and test-runner configs such as
`pytest.ini`, `jest.config.*`, `vitest.config.*`) and write a SHA-256 manifest. During the run: `write`/`patch`/
`remove` on a protected path is refused with a loud explanation; shell commands that clearly mutate one (`rm`, `mv`,
`sed -i`, `>`, `git checkout --`, `git rm`) are refused. The model is told up front, in plain text, which paths are
protected. After the run: re-hash; any difference is reported as an integrity event, and protected files are restored
from the manifest snapshot so the submitted repository state is clean (D-019). Mixed files (`package.json`,
`Cargo.toml`, `pyproject.toml`) cannot be write-protected wholesale; changes to their test sections are detected
post-hoc and flagged. **Acceptance:** unit tests for glob resolution, tool refusal, shell detection, manifest diff and
restore; an end-to-end local eval in which the model is instructed to edit a test and the file is unchanged afterwards.

### R-HACK-3 — Telemetry event stream
An append-only JSONL event log written during the run, with token counts taken from provider usage (HACKATHON §16),
never from the model. Events: `run_start`/`run_end`; `model_call` (id, timestamps, model, input/output/total/cached/
reasoning tokens, context size in tokens and messages); `tool_call` (name, arguments, result size and summary,
success, duration); `context` (compaction before/after, composition by role and by source); `agent_state` (iteration,
plan/todo changes, verification, termination reason); `retry`; `error`; `recovery`; `test_run` (command, exit code,
parsed pass/fail counts, duration); `integrity` (R-HACK-2). Behind a `TelemetrySink` with our canonical internal
schema, plus an adapter layer targeting the organizers' `telemetry.schema.json` once published (D-020). Fail open:
a telemetry error never fails the task. **Acceptance:** schema-validated events from a fixture run; a golden JSONL test.

### R-HACK-4 — Standard report
`forge report <evidence-dir>` turns telemetry + transcript + test results + git diff into `report.json` (internal schema
now, adapter to `report.schema.json` later) and `report.md`, covering HACKATHON §18: outcome, execution summary, model
and token usage, context management, tool usage, orchestration, error recovery, testing, repository changes (files,
lines, protected-file status), timeline. Objective values only. **Acceptance:** golden report from a fixture log.

### R-HACK-5 — Evidence bundle and transcript
`forge exec --evidence-dir <dir>` writes: `prompt.txt`, `transcript.json` (full conversation incl. tool calls and
results — the auto-dump currently fires only on `TaskComplete`, so it must also fire on error and interrupt),
`telemetry.jsonl`, `integrity.json`, `diff.patch`, `tests.json`, `report.json`, `report.md`, and a manifest with harness
version, protocol versions and checksums (HACKATHON §30). Secrets redacted (R-SAFE-3, pulled forward).

### R-HACK-6 — Gemini first
The foundation model is Gemini. Capture `thoughtsTokenCount` (currently parsed and dropped at
`forge_app/src/dto/google/response.rs:372`) and `cachedContentTokenCount`; verify every tool schema is accepted by
Gemini function declarations; map "High" reasoning effort correctly; route evals through `google_ai_studio`
(`GEMINI_API_KEY`) or `vertex_ai`; model registry becomes Gemini-only (D-018).

### R-HACK-7 — Verified completion and failure recovery
Detect the repository's test command(s); require a passing test run after the last source edit before the task may
complete (or record explicitly why it could not run); classify failures (compile/build, test assertion, environment/
dependency, tool error, timeout) and inject a targeted recovery hint; record each as a `recovery` event.

### R-HACK-8 — Local hackathon-shaped evaluation
`benchmarks/hackathon/`: small offline fixture repositories, each with an issue, unit and integration tests, and a
runner that mirrors HACKATHON §9: copy repo → integrity manifest → frozen prompt → `forge exec --evidence-dir` →
run the tests independently → verify integrity → report. This replaces TermBench (T0.5) as the primary suite, and
A/Bs run on it against Gemini only.

### R-HACK-9 — Submission layout and interview readiness
Add `telemetry/`, `reporting/`, `configuration/`, `documentation/` at the repository root and a README covering setup,
execution, architecture, dependencies, configuration, major decisions and known limitations (HACKATHON §32). The Rust
workspace stays where it is (moving `crates/` under `harness/` would break every upstream merge, D-004); `harness/`
holds the entry script and a pointer, and the README explains the mapping (D-021). `documentation/ARCHITECTURE.md`
answers every §25 interview question with references to real telemetry. An evaluation prompt template lives in
`configuration/`.

## 3. Re-prioritised backlog

**Tier 1 — required to compete (do first):** R-HACK-1, R-HACK-2, R-HACK-6, R-HACK-3, R-HACK-5, R-HACK-7, R-HACK-8,
T0.8 (schema rules — now also Gemini compatibility), T1.1 (loud truncation), T3.7 (redaction, needed by R-HACK-5).

**Tier 2 — where points are won:** R-HACK-4, R-HACK-9, T1.2/T1.3/T1.4/T1.5 (output shaping + recovery events),
T2.1 (parallel read-only), T2.4 (non-interactive prompt profile — merges into R-HACK-1), T2.6 (tool-call correction),
T3.1–T3.6 (event log, recall, reversible compaction), T3.10–T3.11 (handoff, triggers + cache accounting), T6.1, T6.4.

**Tier 3 — deferred:** M5 protocol server and client (T5.1–T5.7; not evaluated), T4.2/T4.3 OS sandboxes, T0.5
TermBench (replaced by R-HACK-8), T3.8/T3.9 scorer (only with the foundation model, D-017), T6.2, T6.3, T6.5, T6.6.

**Changed:** T4.1 ask-by-default must never apply in the evaluation profile (it would block the one-shot run); keep it
for interactive use only. All `[A/B]` tasks A/B on Gemini on the R-HACK-8 suite; while there is no credit they ship
behind default-off flags and stay unticked (CLAUDE.md principle 6).

## 4. Open questions for the organizers
1. **Is building on an existing open-source harness (a ForgeCode fork) eligible?** Highest-impact question: if not,
   this plan changes entirely. Until answered, every change is documented so the team can explain and defend it.
2. Are additional models or providers allowed (affects any secondary-model scorer)?
3. Internet access during the run (evals clone from GitHub; Forge has `fetch` and web tools)?
4. Gemini endpoint (AI Studio vs Vertex), rate limits, context window, and runtime limits.
5. Will `telemetry.schema.json` / `report.schema.json` be published before the event, so the adapters can be finished?
6. Is restoring protected test files after a detected change acceptable, or must the harness only prevent?
7. Language/toolchain of the evaluation repository (drives test-runner detection defaults).
