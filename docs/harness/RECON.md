# Recon report — verifying the brief against the code

**Date:** 2026-09-20 · **Upstream commit:** `304bf3b` (`chore(deps): update rust crate rand to v0.10.3 (#3924)`, 2026-09-20)
**Scope:** the two pre-implementation deliverables from `START_HERE.md` — (1) milestone/risk table,
(2) verification of RESEARCH.md's "What the open-source code actually does (S1c)" against this checkout.

Method: four parallel read-only passes over the checkout (loop/hooks/output, compaction/persistence/safety,
surfaces/evals/CLI, and a documents-only risk analysis), each required to cite `path:line`. The two
consequential findings were re-verified by hand before being recorded here.

---

## 1. Verdict summary

| Area | Claims checked | Confirmed | Wrong / not found |
|---|---|---|---|
| Compaction, persistence, safety | 14 | 14 | 0 |
| Loop, hooks, tool output, metrics | 17 | 16 | 1 |
| Surfaces, protocol, evals | 5 | 5 | 0 |

RESEARCH.md is accurate about code that **exists** — every numeric constant it asserts
(100k token threshold, 200 messages, 20% eviction, retain-6, 3 tool failures, 100 requests,
300s tool timeout, 2000-line reads, 100+100-line shell clipping, 50k fetch chars) matched the source
exactly. Both of its failures are claims that something **does not exist**.

---

## 2. Finding A — the eval harness is broken (blocks all of M0)

**Severity: blocking.** `TASKS.md` has no task for this, and T0.4, T0.5 and T0.6 all depend on it.

10 of the 14 evals cannot invoke the agent at all:

```yaml
# benchmarks/evals/read_over_cat/task.yml:3
- FORGE_DEBUG_REQUESTS='{{dir}}/context.json' forgee --provider open_router --model {{model}} -p '{{task}}'
```

- **`--provider` / `--model` are not CLI flags.** `grep -n "pub provider\|pub model" crates/forge_main/src/cli.rs`
  returns nothing. They existed once (`0328695e1`, v1.30.0) and were removed in **`b3ec4d17a`**
  ("chore: add a forge_config crate", #2685). The replacements are `forge config set model <provider> <model>`
  (`crates/forge_main/src/cli.rs:667`) and the env vars `FORGE_SESSION__PROVIDER_ID` / `FORGE_SESSION__MODEL_ID`
  (`crates/forge_config/src/reader.rs:276`), which are tested but undocumented.
- **`FORGE_OVERRIDE_PROVIDER` / `FORGE_OVERRIDE_MODEL` are also dead.** `todo_write_usage` uses this third,
  distinct naming scheme; `ForgeConfig` has no `override_provider`/`override_model` field, so it is silently ignored
  and the eval runs against whatever the ambient config happens to be.
- **`FORGE_DEBUG_REQUESTS` *does* work** — see the correction box below.
- `benchmarks/` was last touched 2026-04-10 (`73c46ca69`) — *after* the flag removal — and was not updated.

> **Correction (supersedes the first draft of this section; see D-012).** An earlier pass concluded that
> `FORGE_DEBUG_REQUESTS` "has never existed" because `grep -rn FORGE_DEBUG_REQUESTS crates/` returns zero hits.
> The grep is accurate; the conclusion was wrong. There is no literal because the variable is handled generically:
> `ForgeConfig` has a `debug_requests: Option<PathBuf>` field (`forge_config/src/config.rs:173`) and
> `ConfigReader::read_env()` (`forge_config/src/reader.rs:104-112`) maps every `FORGE_<FIELD>` env var onto
> `ForgeConfig` via `config::Environment::with_prefix("FORGE").prefix_separator("_").separator("__")`. Only `__`
> marks struct nesting, so the single-underscore `FORGE_DEBUG_REQUESTS` lands on the flat `debug_requests` field.
> It is consumed by `write_debug_request` (`forge_infra/src/http.rs:238`). Verified empirically:
> `FORGE_DEBUG_REQUESTS=/tmp/probe.json forge config list` prints `debug_requests = "/tmp/probe.json"`.
>
> **The same generic mechanism makes every `ForgeConfig` field settable per-process**, which is what makes the
> T0.0 repair a TypeScript-only change. Verified: `FORGE_SESSION__PROVIDER_ID` + `FORGE_SESSION__MODEL_ID` populate
> `[session]`, and `FORGE_AUTO_DUMP=json` sets `auto_dump`.

Affected: `commit_no_markdown`, `multi_file_patch`, `patch_exact_match`, `parallel_tool_calls`, `read_over_cat`,
`redundant_cd_with_cwd`, `refactoring_uses_patch`, `search_over_find`, `semantic_search_quality`, `suggest`.

**Why this matters more than it looks.** clap rejects the unknown argument before the agent ever starts, so the
run fails fast and the harness records it as a normal task failure. Verified empirically:

```
$ forge --provider open_router --model anthropic/claude-sonnet-4.5 -p 'hi'
error: unexpected argument '--provider' found
$ echo $?
2
```

`benchmarks/task-executor.ts:144` rejects on any nonzero exit, so a T0.6 baseline would report ~0% pass and look
like a catastrophic regression rather than an invocation that never reached the agent. → new task **T0.0**.

### Mechanism status, all verified empirically against `target/debug/forge`

| Mechanism | Used by | Status |
|---|---|---|
| `--provider` / `--model` CLI flags | 10 evals | **Dead** — clap exits 2 |
| `FORGE_OVERRIDE_PROVIDER` / `FORGE_OVERRIDE_MODEL` | `todo_write_usage` | **Dead** — no such `ForgeConfig` fields; silently ignored |
| `FORGE_DEBUG_REQUESTS` | 11 evals | **Works** — generic env→config mapping |
| `FORGE_SESSION__PROVIDER_ID` / `FORGE_SESSION__MODEL_ID` | `sem_search` | **Works** — the parallel-safe replacement |
| `FORGE_AUTO_DUMP=json` | nothing yet | **Works** — richer, provider-agnostic alternative |

So the repair is a find-and-replace in `benchmarks/evals/*/task.yml`, with no Rust change:
`--provider X --model Y` → `FORGE_SESSION__PROVIDER_ID=X FORGE_SESSION__MODEL_ID=Y`, and the same for the
`FORGE_OVERRIDE_*` pair. Only `echo` needs no provider at all; the other 13 evals genuinely invoke the agent.

### Two caveats that survive the repair

1. **`debug_requests` writes JSONL, and it captures requests only — never responses.** Each outgoing provider
   request body is appended as one line. The evals `jq` the file as though it were a single document; `jq` does
   process multi-document input, and with `-e` the exit status reflects the *last* output, which happens to be the
   fullest request (each request re-sends the whole history). It works, but by luck rather than design. Because
   nothing is POSTed after the final turn, the file can never contain the final assistant message — no
   `context.json`-based eval needs it today, but T0.3 should not assume this file can supply it.
2. **The `jq` filters assume the OpenAI wire shape** (`.messages[].tool_calls[].function.name/.arguments`).
   Anthropic's native format uses `content` blocks with `tool_use`, so an eval routed through the `anthropic`
   provider directly (rather than through `open_router`) would yield empty arrays and **pass or fail silently for
   the wrong reason**. This directly threatens R-EVAL-1's "≥2 model families" requirement: the Anthropic arm must
   either route via an OpenAI-compatible gateway or use `FORGE_AUTO_DUMP=json`, whose `Context` structure is
   provider-agnostic (`forge_domain/src/context.rs` — ordered messages, `ToolCallFull` with parsed arguments,
   `ToolResult` with output, plus the final assistant message). **Recommended for T0.4.**

### Supporting gaps in the same area

- **No CI runs the evals.** `.github/workflows/` has 7 workflows; the only benchmark-shaped job in `ci.yml` is
  `zsh_rprompt_perf`, an unrelated shell-plugin latency check. `CLAUDE.md` principle 6 ("no efficiency or
  behaviour change ships without an A/B report") has no automation behind it today. → affects T0.9.
- **Model naming is not standardized.** Across `benchmarks/evals/*/task.yml` the same models appear as
  `claude-sonnet-4-5-20250929`, `anthropic/claude-sonnet-4.5`, `anthropic/claude-sonnet-4.6` and bare `glm-4.7`.
  R-EVAL-1 requires "model family" as an A/B dimension; it is not usable as one until this is fixed.
- **The generic LLM-judge is declared but unimplemented.** `benchmarks/model.ts` declares a `type: "llm"`
  validation variant; `benchmarks/verification.ts::runValidations` has no branch for it. The only working judge is
  the bespoke Vertex-AI-gated `benchmarks/evals/semantic_search_quality/llm_judge.ts`, invoked out-of-band via a
  `type: shell` validation. If R-EVAL-3/4 want LLM-judged evals generically, that must be built.
- **No seeds, no repeats, no cost accounting.** `benchmarks/model.ts::Task` has no `seed` or `repeat` field;
  `parallelism` controls concurrency only. `TaskResult` carries wall-clock `duration` and pass/fail, nothing else.
  R-EVAL-1's "k seeds each (default k=3)" is built from zero.

---

## 3. Finding B — R-LOOP-4's premise is false

> RESEARCH.md: "No non-interactive profile found in open source. → `R-LOOP-4`"

A one-shot non-interactive mode already exists and runs through the **same** `Orchestrator::run` loop:

- `crates/forge_main/src/cli.rs:21` — `#[arg(long, short = 'p')] pub prompt: Option<String>`, documented as
  "executes a single command and exits instead of starting an interactive session"; stdin piping supported.
- `crates/forge_main/src/cli.rs:75` — `Cli::is_interactive()` returns false when `prompt`, `piped_input` or a
  subcommand is present.
- `crates/forge_main/src/ui.rs:377` — calls `on_message` once (racing Ctrl+C), then returns.

So `max_requests_per_turn`, doom-loop detection and compaction all already apply in `-p` mode. **The real gap is
narrower and different from what the spec assumes:**

1. **Output is not machine-readable.** `-p` streams markdown/ANSI through the same renderer as interactive mode.
   The existing `--porcelain` flag is wired only to metadata subcommands (`info`, `list`, `config`,
   `conversation stats`, `mcp list`) — never to the chat path.
2. **The exit code carries no task outcome.** `run_inner` returns `Ok(())` and the process exits **0** whether the
   task succeeded, hit `MaxToolFailurePerTurnLimitReached`, or hit `MaxRequestPerTurnLimitReached`. Those two
   reasons already ride on `ChatResponse::Interrupt` (`crates/forge_domain/src/chat_response.rs`), so T0.3 should
   wire the existing variants into an exit code rather than invent new state.

T0.3 is therefore "add a structured output layer and outcome-aware exit codes to an existing dispatch path,"
not "build `forge exec` from nothing." Correspondingly, T2.4's remaining work is the *prompt variant* (disabling
clarification/followup), not the entry point.

---

## 4. Other findings that change implementation plans

| # | Finding | Evidence | Affects |
|---|---|---|---|
| 1 | `task` (subagent) calls bypass `ToolCallStart`/`ToolCallEnd` **and** both toolcall lifecycle hooks — only non-task calls fire them. DoomLoopDetector is blind to subagent calls today. | `forge_app/src/orch.rs:58-172` | T2.1, any telemetry work |
| 2 | The parallel/sequential split is a literal case-insensitive name match on `"task"`. There is no notion of tool independence or side-effect-freedom anywhere. | `forge_app/src/orch.rs:82` | T2.1 |
| 3 | `truncate_shell_output` never receives the command string, so there is no command-type information at truncation-decision time. The string exists on `output.output.command` but is used only for display. | `forge_app/src/truncation/truncate_shell.rs`, call site `operation.rs:596` | T1.4, T1.5 |
| 4 | `session_metrics.rs::Metrics` has exactly four fields (`started_at`, `file_operations`, `files_accessed`, `todos`). Nothing to extend. It persists inside `Conversation`, so adding cost fields likely touches the diesel schema too. | `forge_domain/src/session_metrics.rs:12-28` | T0.2 |
| 5 | `Compact::default()` in Rust is a **no-op config** (`retention_window: 0`, thresholds `None`). The documented numbers live only in `forge_config/.forge.toml`. A hand-built `Compact` in a test silently performs no compaction. | `forge_domain/src/compact/compact_config.rs:106-127` vs `.forge.toml:61-67` | T3.4+ |
| 6 | The allow-everything policy is **materialized to disk on first run** by `init_policies`, not merely an in-memory default. Ask-by-default has a wider blast radius than the spec implies. | `forge_services/src/policy.rs:111-131` | T4.1 |
| 7 | A full-context insta snapshot of `compact()` already exists (`..._render_summary_frame_snapshot-2.snap`). T3.4's golden test should extend it rather than start fresh — but check whether its fixture exercises multi-turn reasoning. | `forge_app/src/compact.rs:495-530` | T3.4 |
| 8 | Hook wiring is broader than RESEARCH.md describes: `on_start` (tracing + title) and `on_toolcall_start`/`on_toolcall_end` (tracing) are also wired. Six lifecycle points, not three. | `forge_app/src/app.rs:162-172` | T6.3 |
| 9 | `protoc` is an undocumented build prerequisite; `forge_repo` carries `tonic`/`prost` and a real `.proto`. | `crates/forge_repo/build.rs:2` | T0.1, CI |
| 10 | Only one diesel table exists (`conversations`); two others were added and later dropped. No event-log-like structure has ever existed. Clean slate for M3. | `forge_repo/src/database/schema.rs:3-13` | T3.1 |

---

## 5. Milestones and risks

| Milestone | Name | Tasks | Requirement IDs | Biggest single risk |
|---|---|---|---|---|
| M0 | Foundations | T0.1–T0.9 | R-EVAL-1/2/3/4, R-PROTO-7, R-TOOL-1 | The baseline is measured before the non-interactive *prompt profile* (R-LOOP-4 → T2.4) exists, so unattended runs can stall on clarifying questions — the exact failure RESEARCH.md cites as the #1 cause of the 25% baseline — contaminating every later comparison. |
| M1 | Output shaping | T1.1–T1.5 | R-OUT-1/2/3/4, R-EVAL-2 | The classifier misfiles a source-like command as noise and strips a real error. The <2% recovery-rate gate cannot catch this: the model never knows to re-fetch, it just answers wrong. |
| M2 | Orchestration | T2.1–T2.6 | R-LOOP-1/2/3/4, R-TOOL-3 | Background jobs + concurrency + interrupts produce orphaned processes, and "no orphan processes" is an explicit, hard-to-automate acceptance bar. |
| M3 | Context engine | T3.1–T3.12 | R-CTX-1…8, R-SAFE-3 | Turning `conversations.context` into a projection over a new event log must keep old conversations resumable *and* replay byte-for-byte — a persistence change whose bugs only appear on real pre-migration data. |
| M4 | Safety | T4.1–T4.3 | R-SAFE-1/2 | The sandbox probe fails open by design, so `--isolate` can be a silent no-op on unsupported kernels while the user believes they are sandboxed. |
| M5 | Protocol | T5.1–T5.7 | R-PROTO-1…7 | The translation layer (T5.2) is built two tasks before the conformance tests that would validate it (T5.6), so under-mapped events surface late, by replay. |
| M6 | Prompts, extensibility, memory | T6.1–T6.6 | R-PROMPT-1/2, R-EXT-1, R-MEM-1, R-TOOL-4, R-CTX-9 | T6.2 repeats the exact experiment GitHub regressed on (S5, Change 3): compressed parallel-agent guidance silently made agents run sequentially despite a clean offline pass. |

### Cross-cutting risks

1. **A/B spend has no project-total visibility.** 15 tasks carry `[A/B]` (T1.2, T1.4, T1.5, T2.1, T2.3, T2.4, T2.5,
   T2.6, T3.5, T3.6, T3.9, T3.11, T3.12, T6.2, T6.6), each needing two model families. The USD 25 guardrail is
   *per-run*; nothing tracks the cumulative total, and re-runs after a fix-and-retest cycle are unbudgeted.
2. **Nothing re-audits M0's instrumentation.** A miscount in `task_metrics.rs` (e.g. `cached_input_tokens`) would
   silently invalidate every A/B from M1 through M6. No task schedules a later audit once the counters are trusted.
3. **D-004 "stay mergeable" collides with the least-additive milestones.** M3 edits the persistence write path,
   T2.1 rewrites `execute_tool_calls`' concurrency model, M5 reads internal `ChatResponse` events. DECISIONS.md
   commits to rebasing at each milestone boundary, but no task in `TASKS.md` owns or verifies that rebase.
4. **Model-version drift.** `baseline.md` pins model versions at M0. If those are deprecated or silently updated
   behind a tag before M6, later A/Bs compare against a baseline that no longer reflects reality. No re-baselining
   task exists.

### Sequencing defects in the backlog

| Defect | Detail |
|---|---|
| **T0.7 cannot meet its own acceptance criteria** | R-EVAL-4's required behaviours include ones marked "(after R-LOOP-1)", "(after R-LOOP-2)" and recall-usage — those are T2.1, T2.2 and T3.3, in M2 and M3. T0.7 in M0 cannot include them, and no follow-up task exists to add them. |
| **T0.4/T0.6 have an undeclared dependency on T2.4** | See the M0 risk above. TASKS.md declares T0.4's dependency on T0.2/T0.3 but not this one. |
| **T0.9's "every task below" clause is invisible** | New tools appear in T2.2 (`job_output`/`job_wait`), T3.3 (`recall`) and T6.4 (`memory_write`); none of those task lines cite R-EVAL-3 or R-TOOL-2, so the micro-eval obligation is easy to miss. |
| **R-TOOL-4 (T6.5) runs backwards** | It consolidates per-model defaults for truncation wording (M1) and reasoning schedule (M2) — which must be implemented before the config exists, then retroactively refactored. |
| M3's internal ordering | Sound. T3.7 (redaction) correctly precedes T3.8/T3.9 (scorers), matching R-CTX-5. |

---

## 6. Proposed spec edits

Recorded as D-008 … D-011 in `DECISIONS.md`.

1. **R-LOOP-4 / RESEARCH.md S1c "Surfaces":** replace "No non-interactive profile found in open source" with a
   description of `forge -p` and the actual gap (machine-readable output + outcome-aware exit code).
2. **RESEARCH.md S1c "Evals":** add that 10 of 14 evals cannot currently invoke the binary, with the `b3ec4d17a`
   reference, and that `benchmarks/` runs in no CI workflow.
3. **`TASKS.md`:** add **T0.0** (repair the eval harness) ahead of T0.4; add a follow-up task to complete T0.7's
   suite once T2.1/T2.2/T3.3 land.
4. **SPEC R-CTX-2 / compaction notes:** note that compaction's numeric defaults live in `forge_config/.forge.toml`,
   not in `Compact::default()`, which is a no-op config.
