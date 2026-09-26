# Task backlog

Work top to bottom. One task = one branch = one PR-sized change. Don't start a task until
its dependencies are ticked. Tick a box only when the SPEC "definition of done" is met, and
append the A/B report path where one is required.

Legend: **[A/B]** requires an A/B report before ticking.

## MH — Hackathon track (governs; see `HACKATHON.md`, `ALIGNMENT.md`, D-017…D-022)
Tier 1 items come first; `[A/B]` now means an A/B on Gemini on the MH.8 suite.
- [x] **TH.1** `R-HACK-1` one-shot autonomy: no reachable interactive prompt in `exec`; fail fast on missing config; wall-clock/request budgets.
      → a test per path, the last two under a real TTY: permission confirm refused (D-053), continue-anyway → `request_limit`.
- [x] **TH.2** `R-HACK-2` test-integrity guard: protected-path manifest, tool + shell refusal, loud notice to the model, post-run verify + restore.
      → complete with mixed files: test sections of package.json/pyproject/Cargo/setup.cfg flagged as `test_config_changed` (D-054).
- [x] **TH.3** `R-HACK-6` Gemini first: capture thoughts/cached tokens, tool-schema compatibility, reasoning-effort mapping, Gemini registry.
      → all items verified in code (D-055); "Gemini-only" superseded by D-049; max_tokens left at 20480 on the evidence.
- [x] **TH.4** `R-HACK-3` telemetry event stream (`TelemetrySink`, internal schema, organizer adapter stub).
      → wired (D-033): run, agent_state, model_call, tool_call, retry (with billed usage), context_compaction,
      integrity, prompt_suppressed, test_run (TH.6), recovery, error, context_composition (D-047); sink redacts;
      title call removed in exec (D-045). **Open:** only the organizer adapter (waits on the published schema, D-020).
- [x] **TH.5** `R-HACK-5` evidence bundle (`--evidence-dir`), transcript on every exit path, redaction (pulls T3.7 forward).
      → bundle on completed/error/time-budget paths, redacted, checksummed (D-034); `tests.json` is the harness's
      own final run (D-037); SIGINT/SIGTERM → exit 5 `interrupted` with the full bundle (D-038).
      SIGKILL: provisional `incomplete` manifest + `integrity.baseline.json` written at start (D-056).
- [x] **TH.6** `R-HACK-7` verified completion: test-command detection, green-after-last-edit gate, failure classification + recovery hints.
      → detection, classification, gate (2 nudges, voluntary stops only), `test_run` + `recovery` events, final
      harness run into `tests.json`, hints for environment/compile/timeout failures (D-037). Known gap: shell-made
      edits don't arm the gate. Gate/hints ship on in exec without an A/B (C2), switchable off.
- [x] **TH.7** `R-HACK-8` local hackathon-shaped evaluation suite and runner (replaces T0.5 as primary suite).
      → 6 fixtures (3 harder, D-049) with issue, unit and — for the harder three — integration tests (D-057); run.ts, ab.ts, bakeoff.ts.
- [x] **TH.8** `R-HACK-4` standard report generator (`forge report`).
      → `forge_harness::report`, golden from `fixtures/evidence_min`; generated in every bundle (D-035).
      Organizer `report.schema.json` adapter waits on publication (D-020).
- [x] **TH.9** `R-HACK-9` submission layout, README, `documentation/ARCHITECTURE.md`, prompt template; `harness/peach-ice-tea` entry command and the `peach-ice-tea` harness identity constant (D-023).
      → wrapper (gemini-only, evidence always, budget, key hygiene), build/check scripts, §32 tree, README, ARCHITECTURE
      answering §25 with code paths + telemetry fields (D-044). ARCHITECTURE now cites a committed real run's telemetry
      (`documentation/evidence/2026-09-27-make-run-py-bugfix/`, D-080). Root `Makefile` (setup/run/test/check/clean, `AI_API_KEY`) verified from a clean clone (D-070).
- [ ] **TH.10** `R-HACK-10` runtime verification gate: before a voluntary stop is accepted, the harness runs the tests itself
      and sends the real failure back. **[A/B]**
      → built behind config `runtime_verify_gate` (default false); replaces the soft gate when on; re-verifies integrity
        after each gate run; capped at 2 attempts; e2e tests (D-083). A/B pending.

## BR — Handoff brief remainder (`AGENT_HANDOFF_BRIEF.md`, D-081…D-084)
Tiers 0–1 done (D-081, D-082, D-083); Tier 2's offline item done (D-084). What remains needs model requests or is Tier 3.
- [ ] **BR.1** Tier 2 A/Bs, one per flag on the default model, k ≥ 2, ship or revert each:
      compact tool docs (D-039; strongest prior, −17.6% input on DeepSeek), T1.2, T1.4, T1.5, T2.1, T2.6, T3.10, T2.7, TH.10.
      → unblocked by the NIM pool (D-085). Done: compact tool docs — shipped on (2 families). Running: TH.10 on Kimi.
- [ ] **BR.2** Tier 3.1 failure-attribution tag on `recovery` events (+ emit `offload_read`/`rerun`/`reread` as events).
      → not started (Tier 3; after BR.1 per the brief's order).
- [ ] **BR.3** Tier 3.2 `write_note` scratchpad tool kept outside `Context.messages` (`R-CTX-10`).
      → not started (Tier 3). Note D-067's concern does not apply: notes live in the event log, not the repo.
- [ ] **BR.4** Tier 3.3 remove 6 dead schema structs from `catalog.rs`.
      → not started (Tier 3, cleanup only).

## MM — Any model (D-049)
- [x] **MM.1** Model profiles: `openrouter-nemotron` (main) and `openrouter-routed` (main + cheap `sage`/compaction);
      `harness/peach-ice-tea --profile`; runner key-stripping covers them.
      → `openrouter` (any model, default Nemotron Ultra free, D-051), `openrouter-routed`; wrapper `--profile/--model`.
- [ ] **MM.2** Model bake-off: every callable model × TH.7 suite × 1 seed → `benchmarks/reports/models/`; choose the default profile.
      → round 1 done (`benchmarks/reports/models/20260926-round1.md`, D-051); 3 harder fixtures added. Round 2 needs account credit.
- [ ] **MM.3** `R-TOOL-4` per-role routing in the runtime (`harness.roles.<agent>` provider/model, compaction model on
      another provider), failing open to the session model. **[A/B]** routed vs single-model.
      → `roles` config + resolver; two-provider end-to-end proof; loud fail-open when a role model fails (D-052).
      Compaction calls no model, so it is not routable; the scorer role waits on T3.9. Unticked pending the A/B.
- [x] **MM.4** Quota/outage failover: when D-040's detector sees an exhausted quota or a provider is unreachable, continue
      on the next model in the profile's fallback list, with a `recovery` event, instead of exiting.
      → `FORGE_HARNESS_FALLBACK_MODELS`; quota + exhausted-retry cases fail over, a 400 does not; e2e tests (D-072).
        Opt-in in profiles until an A/B; `model_failover_count` in the report (D-081).
- [ ] **MM.5** Second-family A/Bs for the flagged work: compact tool docs (DeepSeek arm done), T2.1, T2.6, T1.2.
      → blocked on budget: needs k = 3 A/B runs on a second model family; free tier only (D-069), after the bake-off (D-071).

## M0 — Foundations (nothing else starts until M0 is done)
- [ ] **T0.0** Repair the eval harness so it can invoke the agent at all. 10 of 14 evals use the
      `--provider`/`--model` flags removed in `b3ec4d17a` (clap exits 2), and `todo_write_usage` uses
      `FORGE_OVERRIDE_PROVIDER`/`FORGE_OVERRIDE_MODEL`, which map to no config field and are silently
      ignored. Replace both with `FORGE_SESSION__PROVIDER_ID` / `FORGE_SESSION__MODEL_ID` (parallel-safe:
      process env only, no shared config-file write). Keep `FORGE_DEBUG_REQUESTS` — it works (D-012).
      Standardize model naming across `task.yml` files so "model family" is usable as an A/B dimension.
      TypeScript-only; no Rust change. See `RECON.md` §2, `DECISIONS.md` D-011, D-012, D-013, D-014.
      **Blocks T0.4, T0.5, T0.6.**
      → Repair done and verified as far as is possible without credentials: all 14 task.yml parse, every
      template variable resolves, the harness runs green on `echo` (5/5, 15/15 validations), the registry
      cross-product substitutes correctly (probe: 4/4), and a repaired invocation now reaches the provider
      layer rather than clap. **Unticked pending a live run once `OPENROUTER_API_KEY` is set.**
- [x] **T0.1** Fork hygiene: add `upstream` remote, record the upstream commit we forked from in
      `DECISIONS.md`, confirm `cargo check` and `cargo insta test` pass on a clean clone,
      document the local dev loop in `docs/harness/DEV.md`.
      → forked from `304bf3b`; 2678 tests pass, 0 fail; `docs/harness/DEV.md`; D-008/D-009.
- [x] **T0.2** `R-EVAL-2` task metrics struct + population + persistence (no recovery events yet).
      → `forge_domain/src/task_metrics.rs`; reviewed adversarially, 2 blocking + 4 should-fix found and fixed.
- [x] **T0.3** `R-PROTO-7` minimal `forge exec` (non-interactive flag + JSON metrics line + exit code).
      Protocol streaming comes later; this unblocks the A/B runner.
      Re-scoped by D-010: `forge -p` already exists and reuses `Orchestrator::run`, so this adds a
      machine-readable output layer and an outcome-aware exit code to that path. `-p` currently always
      exits 0; map `ChatResponse::Interrupt`'s existing `MaxToolFailurePerTurnLimitReached` /
      `MaxRequestPerTurnLimitReached` reasons onto exit codes rather than inventing new state.
      → done and reviewed; remaining autonomy gaps (followup gating, non-interactive init) belong to TH.1.
- [ ] **T0.4** `R-EVAL-1` A/B runner over the existing 14 evals (depends T0.0, T0.2, T0.3).
      → superseded for this project: `benchmarks/hackathon/ab.ts` is the A/B runner over the primary suite (TH.7). The 14 upstream
        evals stay reachable through T0.0's repair; not pursued further (ALIGNMENT §3).
- [ ] **T0.5** `R-EVAL-1` TermBench 2.0 subset suite via Harbor (pick ~30 tasks stratified by
      category; record the list in `benchmarks/suites/termbench-subset.txt`).
      → deferred: replaced by TH.7 as the primary suite (ALIGNMENT §3, Tier 3).
- [ ] **T0.6** Baseline report: run upstream-equivalent config on both suites × 2 model families ×
      3 seeds. Commit as `benchmarks/reports/baseline.md`. **All later A/Bs compare to this
      or to the previous shipped state.** Note: this baseline predates the non-interactive prompt
      profile (`R-LOOP-4`, T2.4), so footnote it — unattended runs can stall on clarifying questions
      and wall-time/success deltas are not comparable once T2.4 ships (`RECON.md` §5).
      → blocked on budget: 2 families × 3 seeds × 2 suites is far beyond 50 free requests/day (D-069). Interim baseline:
        round 1 + free screens (`benchmarks/reports/models/`, D-051, D-071).
- [x] **T0.7** `R-EVAL-4` behavioural regression suite (parallel subagents, read-before-patch,
      todo usage, verification, truncation awareness). Must be green on baseline.
      Note: R-EVAL-4 also requires tests gated on R-LOOP-1/R-LOOP-2/R-CTX-3, which land in M2/M3 —
      those are tracked separately as T3.13, not here (`RECON.md` §5).
- [x] **T0.8** `R-TOOL-1` schema-rule unit test over all tool definitions.
      → flat-schema rule enforced catalog-wide; `required`-before-`properties` deferred with reason (D-055).
      → `benchmarks/hackathon/behaviour.ts` (4 checks) run on 19 real bundles: every completed run passes (D-078);
        subagent check moves to T3.13.
- [ ] **T0.9** `R-EVAL-3` per-tool micro-eval template + CI job reporting per-tool error rate by model.
      Every task below that adds or changes a tool must add its micro-eval and follow `R-TOOL-2` naming.
      → micro-evals `tool_shell`/`tool_read`/`tool_write` (free models) + `tool_errors.ts` report on real runs (D-079);
        not yet run; CI job needs a team decision on a CI key.

## M1 — Output shaping (cheap, measurable wins)
- [x] **T1.1** `R-OUT-4` loud-truncation audit across read/shell/fetch/search/MCP + snapshots.
      → one shared recovery sentence; snapshots for read/shell/search/fetch; MCP shaper finally wired (D-059).
- [ ] **T1.2** `R-OUT-1` line numbers off by default; update fs_read.md / fs_patch.md. **[A/B]**
      → behind `FORGE_HARNESS_LINE_NUMBERS_OFF=1` (default off; explicit `show_line_numbers` honoured), D-039.
      Related, not a task yet: `FORGE_HARNESS_COMPACT_TOOL_DOCS=1` cuts 15.4% of every request.
- [x] **T1.3** `R-EVAL-2` recovery events (`offload_read`, `rerun_same_command`, `reread_same_range`)
      and `R-OUT-3` stable handles on every withheld output.
      → dump files finally registered (offload_read was always 0), MCP handles, first_error_recovered derived (D-060).
- [ ] **T1.4** `R-OUT-2` classifier + search regrouping (lossless parts only). **[A/B]**
      → built behind `FORGE_HARNESS_SEARCH_REGROUP` (default off), unit + e2e tests (D-073). A/B pending.
- [ ] **T1.5** `R-OUT-2` noise compressor with fixture tests for cargo/npm/pytest/jest/tsc/eslint. **[A/B]**
      Ship only if noise-class recovery < 2%.
      → built behind `FORGE_HARNESS_NOISE_COMPRESSION` (default off), fixture + e2e tests (D-073). A/B pending.

## M2 — Orchestration
- [ ] **T2.1** `R-LOOP-1` concurrency classes + concurrent read-only batches; add behaviour test. **[A/B]**
      → built behind `FORGE_HARNESS_PARALLEL_READONLY=1` (default off) with overlap/order specs and a real-binary
      test (D-041). Unticked pending A/B.
- [ ] **T2.2** `R-LOOP-2` job registry + `run_in_background` for shell + `job_output` / `job_wait`.
      → deferred: background jobs pay off on long commands; every fixture's tests run in under a second, so there is nothing
        to measure yet. Revisit with T3.12's long tasks.
- [ ] **T2.3** `R-LOOP-2` batched completion delivery in tool-result format + keep-alive when the
      model stops with jobs running; background subagents. **[A/B]**
      → deferred with T2.2 (depends on it).
- [ ] **T2.4** `R-LOOP-4` non-interactive profile (prompt variant, followup disabled); ambiguous-task eval. **[A/B]**
      → non-interactive half done as TH.1 (followup answered, prompts refused; D-031, D-048, D-053); the prompt-variant half
        is `[A/B]` and waits on budget (D-069).
- [ ] **T2.5** `R-LOOP-3` progressive reasoning schedule behind config; ship on only if A/B wins. **[A/B]**
      → deferred: `[A/B]`; reasoning controls differ across the free models, so it waits for the bake-off's winner (D-069, D-071).
- [ ] **T2.6** `R-TOOL-3` pre-dispatch correction layer + correction counters. **[A/B]**
      → built behind `FORGE_HARNESS_TOOL_CORRECTION=1` (default off): unambiguous key renames, `recovery` events,
      real-binary test (D-042). Unticked pending A/B.
- [ ] **T2.7** `R-LOOP-5` enforced doom-loop escalation ladder (warn → skip → pause). **[A/B]**
      → built behind `FORGE_HARNESS_DOOM_LOOP_ESCALATION` (default off): unit, orchestrator and exec tests (D-082). A/B pending.

## M3 — Context engine
- [x] **T3.1** `R-CTX-1` migrations + `thread_events` + `artifacts` + repository + replay test.
      → domain events + replay, SQLite repo with content-addressed artifacts, cap and GC; byte-exact replay test (D-064).
- [x] **T3.2** `R-CTX-1` write path: orchestrator appends all events; `conversations.context`
      becomes projection; old conversations still resume.
      → every save appends via `EventLogWriter`; content-based diff with `Revise` + compactor-shaped split (D-065).
- [x] **T3.3** `R-CTX-3` `recall` tool over artifacts; stub format with handles.
      → recall handles as files read with `read`/`fs_search` (no new tool), listed in the summary; behind `FORGE_HARNESS_RECALL_HANDLES` (D-066).
- [x] **T3.4** `R-CTX-2` pipeline skeleton with S3 = existing Compactor (behaviour identical to
      today; golden test proves it).
      → `compaction_pipeline::Pipeline`, hook wired, golden test over 4 shapes with a non-vacuity guard (D-062).
- [ ] **T3.5** `R-CTX-2` S0 supersede. **[A/B]**
      → built behind `FORGE_HARNESS_SUPERSEDE` (default off): re-read/edit/re-run; narrower-search rule deferred (D-075). A/B pending.
- [ ] **T3.6** `R-CTX-2` S1 offload. **[A/B]**
      → built behind `FORGE_HARNESS_OFFLOAD` (default off); spares S3 when enough; e2e proven (D-074). A/B pending.
- [x] **T3.7** `R-SAFE-3` redaction utility (needed before any scorer sends data).
      → `forge_harness::redact` (36 tests); applied to evidence files, telemetry free text and scorer previews.
      External hooks (T6.3) must call it when built.
- [x] **T3.8** `R-CTX-4` scorer trait + HeuristicScorer + fake-scorer tests.
      → trait, fail-open plan builder, heuristic incl. the missing "referenced later" signal (D-061); wiring is T3.4.
- [ ] **T3.9** `R-CTX-4` LlmScorer (port of save-token-jev core) + `R-CTX-5` fail-open gate. **[A/B]**
      → S2 stage wired with the heuristic scorer behind `FORGE_HARNESS_SCORE_STAGE` (fail-open gate via `build_plan`);
        `LlmScorer` deferred: a model request per compaction does not fit the free tier (D-069, D-077).
- [x] **T3.10** `R-CTX-6` handoff note in S3.
      → deterministic note atop the S3 summary behind `FORGE_HARNESS_HANDOFF_NOTE` (default off, A/B pending; D-063).
- [ ] **T3.11** `R-CTX-7` soft/hard triggers + `R-CTX-8` cache accounting. **[A/B]**
      → soft trigger (S0+S1 only) behind `FORGE_HARNESS_SOFT_COMPACTION`; cache rate around compactions in the report (D-076). A/B pending.
- [ ] **T3.12** Long-horizon eval suite (≥ 60-turn tasks) and final context-engine A/B. **[A/B]**
      → blocked on budget: one ≥ 60-turn run exceeds a day's 50 free requests (D-069).
- [ ] **T3.13** `R-EVAL-4` completion: extend the T0.7 behavioural suite with the tests that could not
      exist in M0 — parallel independent read-only tools (after T2.1), background job usage for long
      commands (after T2.2), and recall-when-needed (after T3.3). Closes the gap in `RECON.md` §5.
      → deferred: its checks need real runs that use subagents, background jobs (T2.2) and recall (flagged, D-066); T0.7's
        subagent check moved here (D-078).

## M4 — Safety
- [ ] **T4.1** `R-SAFE-1` ask-by-default policy + TUI approval prompt + `--yolo`.
      → deferred: interactive-only by D-022 (an unattended judged run must never ask); Tier 3.
- [ ] **T4.2** `R-SAFE-2` Linux Landlock/bwrap isolation behind `--isolate`.
      → deferred: Tier 3 (ALIGNMENT §3); the organisers provide the evaluation sandbox.
- [ ] **T4.3** `R-SAFE-2` macOS sandbox-exec profile.
      → deferred: Tier 3, as T4.2.

## M5 — Protocol
- [ ] **T5.1** `R-PROTO-1` `forge_protocol` crate: initialize, thread, turn, item types; schemars.
      → deferred: Tier 3 — the protocol server is not evaluated (ALIGNMENT §3).
- [ ] **T5.2** `R-PROTO-2/3` `forge_app_server` runtime: stdio JSONL, thread manager, translation
      layer from `ChatResponse`; thread start/resume/list/read; turn start/interrupt; item lifecycle.
      → deferred: Tier 3, with T5.1.
- [ ] **T5.3** `R-PROTO-3` fork/archive using the event log.
      → deferred: Tier 3, with T5.1 (the event log it needs now exists, D-064).
- [ ] **T5.4** `R-PROTO-4` approvals as server requests wired to the policy engine.
      → deferred: Tier 3, with T5.1.
- [ ] **T5.5** `R-PROTO-5` codegen commands + CI check for generated files.
      → deferred: Tier 3, with T5.1.
- [ ] **T5.6** `R-PROTO-6` TS reference client + recorded-session conformance tests.
      → deferred: Tier 3, with T5.1.
- [ ] **T5.7** `R-PROTO-7` `exec --json` streams protocol notifications.
      → deferred: Tier 3, with T5.1 (`exec --json` already emits the outcome line, T0.3).

## M6 — Prompts, extensibility, memory
- [x] **T6.1** `R-PROMPT-1` per-component prompt token report.
      → `context_composition` at each conversation's first request (by role and source) + report line (D-047).
- [ ] **T6.2** `R-PROMPT-2` compress `task.md` behind behaviour tests. **[A/B]**
      → deferred: Tier 3 and `[A/B]` (D-069).
- [ ] **T6.3** `R-EXT-1` external hooks incl. `pre_compact` + save-token-jev-compatible example.
      → deferred: Tier 3; redaction for hook payloads is ready (T3.7).
- [ ] **T6.4** `R-MEM-1` project memory file + `memory_write` tool.
      → memory file loaded (bounded, loud clip); `memory_write` deferred: it would edit the judged repo (D-067).
- [ ] **T6.5** `R-TOOL-4` model profiles (Anthropic, OpenAI) consolidating per-model defaults.
      → deferred until the bake-off gives per-model evidence (D-071); per-provider profiles and per-role models exist
        (`configuration/profiles/`, MM.3).
- [ ] **T6.6** `R-CTX-9` entry-point discovery hint. **[A/B]**
      → deferred: Tier 3 and `[A/B]`.
