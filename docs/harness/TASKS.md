# Task backlog

Work top to bottom. One task = one branch = one PR-sized change. Don't start a task until
its dependencies are ticked. Tick a box only when the SPEC "definition of done" is met, and
append the A/B report path where one is required.

Legend: **[A/B]** requires an A/B report before ticking.

## M0 — Foundations (nothing else starts until M0 is done)
- [ ] **T0.0** Repair the eval harness so it can invoke the agent at all: 10 of 14 evals use the
      `--provider`/`--model` flags removed in `b3ec4d17a`, and 11 rely on `PEACH_DEBUG_REQUESTS`,
      which never existed. Standardize model naming across `task.yml` files. See `RECON.md` §2,
      `DECISIONS.md` D-011. **Blocks T0.4, T0.5, T0.6.**
- [x] **T0.1** Fork hygiene: add `upstream` remote, record the upstream commit we forked from in
      `DECISIONS.md`, confirm `cargo check` and `cargo insta test` pass on a clean clone,
      document the local dev loop in `docs/harness/DEV.md`.
      → forked from `304bf3b`; 2678 tests pass, 0 fail; `docs/harness/DEV.md`; D-008/D-009.
- [ ] **T0.2** `R-EVAL-2` task metrics struct + population + persistence (no recovery events yet).
- [ ] **T0.3** `R-PROTO-7` minimal `peach exec` (non-interactive flag + JSON metrics line + exit code).
      Protocol streaming comes later; this unblocks the A/B runner.
      Re-scoped by D-010: `peach -p` already exists and reuses `Orchestrator::run`, so this adds a
      machine-readable output layer and an outcome-aware exit code to that path. `-p` currently always
      exits 0; map `ChatResponse::Interrupt`'s existing `MaxToolFailurePerTurnLimitReached` /
      `MaxRequestPerTurnLimitReached` reasons onto exit codes rather than inventing new state.
- [ ] **T0.4** `R-EVAL-1` A/B runner over the existing 14 evals (depends T0.0, T0.2, T0.3).
- [ ] **T0.5** `R-EVAL-1` TermBench 2.0 subset suite via Harbor (pick ~30 tasks stratified by
      category; record the list in `benchmarks/suites/termbench-subset.txt`).
- [ ] **T0.6** Baseline report: run upstream-equivalent config on both suites × 2 model families ×
      3 seeds. Commit as `benchmarks/reports/baseline.md`. **All later A/Bs compare to this
      or to the previous shipped state.** Note: this baseline predates the non-interactive prompt
      profile (`R-LOOP-4`, T2.4), so footnote it — unattended runs can stall on clarifying questions
      and wall-time/success deltas are not comparable once T2.4 ships (`RECON.md` §5).
- [ ] **T0.7** `R-EVAL-4` behavioural regression suite (parallel subagents, read-before-patch,
      todo usage, verification, truncation awareness). Must be green on baseline.
      Note: R-EVAL-4 also requires tests gated on R-LOOP-1/R-LOOP-2/R-CTX-3, which land in M2/M3 —
      those are tracked separately as T3.13, not here (`RECON.md` §5).
- [ ] **T0.8** `R-TOOL-1` schema-rule unit test over all tool definitions.
- [ ] **T0.9** `R-EVAL-3` per-tool micro-eval template + CI job reporting per-tool error rate by model.
      Every task below that adds or changes a tool must add its micro-eval and follow `R-TOOL-2` naming.

## M1 — Output shaping (cheap, measurable wins)
- [ ] **T1.1** `R-OUT-4` loud-truncation audit across read/shell/fetch/search/MCP + snapshots.
- [ ] **T1.2** `R-OUT-1` line numbers off by default; update fs_read.md / fs_patch.md. **[A/B]**
- [ ] **T1.3** `R-EVAL-2` recovery events (`offload_read`, `rerun_same_command`, `reread_same_range`)
      and `R-OUT-3` stable handles on every withheld output.
- [ ] **T1.4** `R-OUT-2` classifier + search regrouping (lossless parts only). **[A/B]**
- [ ] **T1.5** `R-OUT-2` noise compressor with fixture tests for cargo/npm/pytest/jest/tsc/eslint. **[A/B]**
      Ship only if noise-class recovery < 2%.

## M2 — Orchestration
- [ ] **T2.1** `R-LOOP-1` concurrency classes + concurrent read-only batches; add behaviour test. **[A/B]**
- [ ] **T2.2** `R-LOOP-2` job registry + `run_in_background` for shell + `job_output` / `job_wait`.
- [ ] **T2.3** `R-LOOP-2` batched completion delivery in tool-result format + keep-alive when the
      model stops with jobs running; background subagents. **[A/B]**
- [ ] **T2.4** `R-LOOP-4` non-interactive profile (prompt variant, followup disabled); ambiguous-task eval. **[A/B]**
- [ ] **T2.5** `R-LOOP-3` progressive reasoning schedule behind config; ship on only if A/B wins. **[A/B]**
- [ ] **T2.6** `R-TOOL-3` pre-dispatch correction layer + correction counters. **[A/B]**

## M3 — Context engine
- [ ] **T3.1** `R-CTX-1` migrations + `thread_events` + `artifacts` + repository + replay test.
- [ ] **T3.2** `R-CTX-1` write path: orchestrator appends all events; `conversations.context`
      becomes projection; old conversations still resume.
- [ ] **T3.3** `R-CTX-3` `recall` tool over artifacts; stub format with handles.
- [ ] **T3.4** `R-CTX-2` pipeline skeleton with S3 = existing Compactor (behaviour identical to
      today; golden test proves it).
- [ ] **T3.5** `R-CTX-2` S0 supersede. **[A/B]**
- [ ] **T3.6** `R-CTX-2` S1 offload. **[A/B]**
- [ ] **T3.7** `R-SAFE-3` redaction utility (needed before any scorer sends data).
- [ ] **T3.8** `R-CTX-4` scorer trait + HeuristicScorer + fake-scorer tests.
- [ ] **T3.9** `R-CTX-4` LlmScorer (port of save-token-jev core) + `R-CTX-5` fail-open gate. **[A/B]**
- [ ] **T3.10** `R-CTX-6` handoff note in S3.
- [ ] **T3.11** `R-CTX-7` soft/hard triggers + `R-CTX-8` cache accounting. **[A/B]**
- [ ] **T3.12** Long-horizon eval suite (≥ 60-turn tasks) and final context-engine A/B. **[A/B]**
- [ ] **T3.13** `R-EVAL-4` completion: extend the T0.7 behavioural suite with the tests that could not
      exist in M0 — parallel independent read-only tools (after T2.1), background job usage for long
      commands (after T2.2), and recall-when-needed (after T3.3). Closes the gap in `RECON.md` §5.

## M4 — Safety
- [ ] **T4.1** `R-SAFE-1` ask-by-default policy + TUI approval prompt + `--yolo`.
- [ ] **T4.2** `R-SAFE-2` Linux Landlock/bwrap isolation behind `--isolate`.
- [ ] **T4.3** `R-SAFE-2` macOS sandbox-exec profile.

## M5 — Protocol
- [ ] **T5.1** `R-PROTO-1` `peach_protocol` crate: initialize, thread, turn, item types; schemars.
- [ ] **T5.2** `R-PROTO-2/3` `peach_app_server` runtime: stdio JSONL, thread manager, translation
      layer from `ChatResponse`; thread start/resume/list/read; turn start/interrupt; item lifecycle.
- [ ] **T5.3** `R-PROTO-3` fork/archive using the event log.
- [ ] **T5.4** `R-PROTO-4` approvals as server requests wired to the policy engine.
- [ ] **T5.5** `R-PROTO-5` codegen commands + CI check for generated files.
- [ ] **T5.6** `R-PROTO-6` TS reference client + recorded-session conformance tests.
- [ ] **T5.7** `R-PROTO-7` `exec --json` streams protocol notifications.

## M6 — Prompts, extensibility, memory
- [ ] **T6.1** `R-PROMPT-1` per-component prompt token report.
- [ ] **T6.2** `R-PROMPT-2` compress `task.md` behind behaviour tests. **[A/B]**
- [ ] **T6.3** `R-EXT-1` external hooks incl. `pre_compact` + save-token-jev-compatible example.
- [ ] **T6.4** `R-MEM-1` project memory file + `memory_write` tool.
- [ ] **T6.5** `R-TOOL-4` model profiles (Anthropic, OpenAI) consolidating per-model defaults.
- [ ] **T6.6** `R-CTX-9` entry-point discovery hint. **[A/B]**
