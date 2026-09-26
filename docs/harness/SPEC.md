# Harness specification

**Goal:** turn our fork of an open-source coding agent into a best-in-class coding-agent harness: keep Peach's
tool-reliability lead and add what it lacks — cost-aware output shaping, efficient
orchestration, reversible context management, a client protocol, safe defaults, and an
evaluation system that measures cost per *completed task*.

Every requirement has an ID, a **why** (pointer into `RESEARCH.md`), a **where** (code
location in this repo, paths relative to `crates/`), and **acceptance** criteria.
`TASKS.md` orders the work. `DECISIONS.md` records architectural choices.

---

## 0. Design principles (apply to every change)

1. **Optimise the completed task, never the single call.** A change that shrinks a tool
   result but adds turns is a regression. (S5)
2. **Lossless before reversible before lossy.** Remove formatting first; then offload with a
   retrieval path; summarise only as a last resort. (S3, S5)
3. **Enforce in the runtime what matters; don't hope the prompt does it.** (S1a FM6, S1b Fix 4)
4. **Make important facts loud.** Truncation, offloading and compaction must be stated in
   plain text in the model's context, not only in metadata. (S1b Fix 3)
5. **Fail open.** Any optional subsystem (scorer, hook, classifier, sandbox probe) that errors
   or times out falls back to today's behaviour and logs why. (S2)
6. **Evidence is local.** Every behaviour or efficiency change ships with an eval report on
   our workloads, across at least two model families. (S5, S1b)
7. **Stay mergeable with upstream.** Prefer new modules and additive changes over rewrites of
   upstream files; keep crate names. (D-004)
8. **Follow the repo's `AGENTS.md` Rust conventions** (except its commit-trailer rule).

---

## 1. Evaluation and measurement — `R-EVAL`  *(build first)*

### R-EVAL-1 — Paired A/B runner with task-level cost
- **Why:** S5 method (offline benchmark → online experiment; per-surface evidence); S1a
  "thousands of evaluations".
- **Where:** extend `benchmarks/` (TypeScript). New `benchmarks/ab.ts`.
- **What:** run *baseline* (a git ref or config profile) vs *candidate* on the same task set,
  k seeds each (default k=3), ≥2 model families (one Anthropic, one OpenAI by default).
  Output a markdown report with, per arm: success rate (with 95% CI), mean and median
  input tokens, cached input tokens, output tokens, LLM calls, tool calls, wall time,
  recovery events (R-EVAL-2), and deltas with significance.
- **Acceptance:** `npm run ab -- --base <ref> --cand <ref> --suite <name>` produces
  `benchmarks/reports/<date>-<name>.md`; report states pass/fail against the change's
  declared success criteria; works with the existing 14 evals and with a TermBench 2.0
  subset (via Harbor) as a second suite.

### R-EVAL-2 — Task metrics in the runtime
- **Why:** `Metrics` has no cost fields (S1c); S5 tracked recovery signals; S1a wants
  recovery rate after first tool error.
- **Where:** new `peach_domain/src/task_metrics.rs`; populate from the orchestrator
  (`peach_app/src/orch.rs`) and tool executor; persist alongside conversation metrics;
  emit as a final structured line in non-interactive/`exec` mode.
- **Fields:** llm_calls, input_tokens, cached_input_tokens, output_tokens, reasoning_tokens,
  tool_calls by tool, tool_errors by tool, wall_ms, compactions (count, tokens before/after,
  stage reached), and **recovery events**:
  `offload_read` (agent read an offloaded/temp output), `recall_call` (R-CTX-3),
  `rerun_same_command` (identical shell command within 5 turns),
  `reread_same_range` (same file+range read twice with no intervening write),
  `first_error_recovered` (task succeeded after its first tool error).
- **Acceptance:** unit tests for each counter; `exec` mode prints a JSON metrics object;
  A/B runner consumes it.

### R-EVAL-3 — Per-tool, per-model reliability and schema-shape evals
- **Why:** S1a FM2/FM3; S1b "schema-shape evals before new tools ship".
- **What:** micro-eval per tool isolating wrong-tool / wrong-args / wrong-sequence, run per
  model. Any new or changed tool schema must pass its micro-eval on both model families
  before merge.
- **Acceptance:** `benchmarks/evals/tool_<name>/` exists for every tool touched by this
  project; CI job reports per-tool error rate by model.

### R-EVAL-4 — Behavioural regression suite
- **Why:** S5 prompt-compression regression (parallel agents became sequential); S1a todo
  enforcement; S1b verification.
- **What:** named behaviour tests that must stay green across every change:
  parallel independent subagents; parallel independent read-only tools (after R-LOOP-1);
  read-before-patch; todo usage on multi-step tasks; verification before finish;
  truncation awareness (reads further when told more lines exist); background job usage
  for long commands (after R-LOOP-2); recall used when needed (after R-CTX-3).
- **Acceptance:** suite runs in < 15 minutes; any PR touching prompts, tool descriptions,
  schemas or the loop must include a green run.

---

## 2. Tool output shaping — `R-OUT`

### R-OUT-1 — Remove vestigial line-number prefixes from whole-file reads
- **Why:** edits are string-match (`fs_patch.md`); S5 saw ~5% offline / ~3% online cost
  reduction removing exactly this with no edit-failure increase.
- **Where:** `peach_domain/src/tools/catalog.rs` (`FSRead::show_line_numbers` default),
  `peach_domain/src/tools/descriptions/fs_read.md` and `fs_patch.md` (remove prefix
  instructions), `peach_app/src/operation.rs` (`FsRead` branch).
- **What:** default `show_line_numbers=false`. Keep the `display_lines` / `total_lines`
  attributes. Ranged reads get a single header line (`lines 120–180 of 900`) instead of
  per-line prefixes. Keep numbers in `fs_search` (`-n`) and diffs. Model may still request
  numbers explicitly.
- **Acceptance:** A/B shows input tokens ↓ with success within noise and patch-failure rate
  not up (patch_exact_match, multi_file_patch, refactoring_uses_patch evals + TermBench
  subset). Update snapshot tests.

### R-OUT-2 — Content-aware shell output shaping
- **Why:** S5 three-part policy; S1c shows uniform head/tail clipping.
- **Where:** new `peach_app/src/truncation/classify.rs` and `truncation/compress_noise.rs`;
  call from the `Shell` branch of `operation.rs::into_tool_output` before
  `truncate_shell_output`.
- **What:** classify each command by parsing the command line (first program in each
  pipeline segment, plus known subcommands):
  - **Source-like / arbitrary** (`cat`, `head`, `tail`, `sed -n`, `bat`, `less`,
    `git diff|show|log -p|blame`, `jq`, running project scripts, unknown programs):
    pass through unchanged up to a *high* hard cap (default 2,000 lines / 200 KB), beyond
    which use head/tail + offload.
  - **Search** (`grep`, `rg`, `ag`, `find`, `fd`, `ls -R`, `git grep`): lossless
    regrouping — group matches under a file header, drop repeated path prefixes; never drop
    a match (cap still applies with offload).
  - **Noise** (package installs, builds, test runners, linters, formatters, progress-bar
    output: `npm|pnpm|yarn|pip|cargo|go|mvn|gradle|make|pytest|jest|vitest|cargo test|eslint|tsc …`):
    strip ANSI and carriage-return progress frames; collapse runs of identical or
    near-identical lines ("… 184 similar lines"); collapse passing test lines to counts;
    **always keep** every line containing error/warn/fail/panic/exception/traceback/
    assert, their surrounding context (±3 lines), and the final summary block. Apply only if
    it saves ≥ 30% and ≥ 2,000 chars; otherwise pass through.
  - Unknown → current head/tail behaviour.
- **Always:** full raw stdout/stderr remain in the temp file (existing `dump_operation`),
  and the result contains a plain-text recovery line (R-OUT-4).
- **Acceptance:** unit tests with real fixture logs per class (cargo, npm, pytest, jest,
  git diff, rg); A/B cost ↓, success within noise, **recovery rate for the noise class
  < 2%** (R-EVAL-2). If recovery for any class exceeds the bar, make that class
  pass-through.

### R-OUT-3 — Recovery path is first-class
- **Why:** S5 recovery path as both safety and metric.
- **What:** every truncated/compressed/offloaded output carries a stable handle (the temp
  file path today; the artifact handle after R-CTX-3). Reading it increments
  `offload_read`.

### R-OUT-4 — Loud truncation everywhere
- **Why:** S1b Fix 3.
- **What:** audit read, shell, fetch, search, MCP results: whenever anything is withheld,
  the body contains one plain sentence stating how much was withheld and the exact call
  to get it (e.g. `… 3,823 more lines not shown. Full output: read <path> with a range.`).
- **Acceptance:** snapshot tests per tool; truncation-awareness behaviour test (R-EVAL-4).

---

## 3. Loop and orchestration — `R-LOOP`

### R-LOOP-1 — Run independent read-only tool calls concurrently
- **Why:** `orch.rs::execute_tool_calls` runs non-`task` calls sequentially although
  prompts request parallel calls (S1c); wall-clock is failure under time limits (S1a FM5/7).
- **Where:** `peach_domain/src/tools/catalog.rs` (add a `concurrency` class per tool),
  `peach_app/src/orch.rs`.
- **What:** tools classed `ReadOnly` (read, fs_search, sem_search, fetch, skill, todo_read,
  MCP tools explicitly annotated read-only) execute concurrently in contiguous batches;
  `Exclusive` tools (write, patch, multi_patch, remove, undo, shell, todo_write, unknown MCP)
  execute alone, in order. Preserve: result order, `ToolcallStart`/`ToolcallEnd` hooks per
  call, the UI `Notify` handshake (send all starts for a batch, await acks, run, send ends
  in order), per-turn error accounting.
- **Acceptance:** behaviour test shows concurrent execution; existing tests green; wall
  time ↓ on the suite; no ordering bugs in the TUI.

### R-LOOP-2 — Background jobs with batched completion delivery
- **Why:** S5 change 4 (~2.3%); `tool_timeout_secs=300` blocks the loop (S1c).
- **Where:** new `peach_services/src/tool_services/jobs.rs` (job registry) +
  `shell` tool arg; `task` tool arg; orchestrator integration in `orch.rs`.
- **What:**
  - `shell` gets `run_in_background: bool`; returns immediately with `job_id`, command and
    a note. `task` gets the same flag for subagents.
  - New tools: `job_output(job_id, tail_lines?)` (non-blocking, current output + status)
    and `job_wait(job_ids[], timeout_secs)` (blocks until any/all complete).
  - **Delivery:** when jobs complete and the model is not explicitly waiting on them, the
    harness collects completions within a short debounce window (default 300 ms) and
    injects them **as tool results in the existing tool-result format** at the start of
    the next request — no extra retrieval turn.
  - If the model finishes its turn while jobs are still running, the loop does not end: it
    waits for the next completion(s), delivers them, and continues.
  - Job outputs go through R-OUT-2 shaping and R-OUT-3 handles.
  - Jobs are killed on session end unless the protocol client opts to keep them.
- **Acceptance:** behaviour test (long test run in background + parallel investigation);
  A/B shows LLM calls ↓ on tasks with long commands; no orphan processes.

### R-LOOP-3 — Progressive reasoning budget (open-source rebuild)
- **Why:** S1a FM7 — high reasoning for the first 10 assistant messages, low afterwards,
  high again on verification; this lives in the upstream vendor's proprietary services, not our fork.
- **Where:** new `peach_app/src/hooks/reasoning_budget.rs` on `on_request`, adjusting
  `Context.reasoning` effort.
- **What:** config `reasoning.schedule = { plan_messages: 10, plan_effort: high,
  exec_effort: low, verify_effort: high }`, per agent and per model; subagents default to
  `exec_effort`. No-op for models without effort control.
- **Acceptance:** A/B on TermBench subset: success ≥ baseline, wall time and reasoning
  tokens ↓. Ship off-by-default if not both true.

### R-LOOP-4 — Non-interactive profile
- **Why:** S1a FM1 (25% baseline was largely from waiting on a human).
- **Where:** CLI flag `--non-interactive` (implied by `exec` mode, R-PROTO-7); agent prompt
  variant; disables `followup`/yield-for-question tools.
- **What:** prompt forbids clarification, instructs to assume reasonable defaults and
  record assumptions in the final message; completion must be explicit.
- **Acceptance:** eval with deliberately ambiguous tasks: zero follow-up requests, success
  ≥ interactive baseline.

---

## 4. Tool interface reliability — `R-TOOL`  *(keep and extend Peach's strength)*

- **R-TOOL-1 — Keep schema rules.** `required` before `properties`; flat schemas; enforced by
  a unit test that walks every tool definition and fails on nested objects with their own
  `required` or on `properties` preceding `required`. (S1b)
- **R-TOOL-2 — Training-prior names.** New tools use names/args common in training data
  (`old_string`/`new_string`, `file_path`, `command`, `pattern`). (S1a FM3)
- **R-TOOL-3 — Pre-dispatch correction layer (open-source rebuild).** Before executing a
  call: coerce near-miss argument names to schema names (edit distance ≤ 2 or known
  aliases), coerce string↔number/bool, resolve relative paths against cwd, reject
  preconditions early with an actionable error (e.g. patch before read). Log each
  correction; count them per tool/model in R-EVAL-2. Where: `peach_app/src/tool_resolver.rs`
  / `tool_executor.rs`, reuse `peach_json_repair`. (S1a Services item 3)
- **R-TOOL-4 — Per-model runtime defaults.** A `model_profiles` config mapping model family →
  defaults (verification enforcement strength, truncation wording, reasoning schedule).
  Start with two profiles (Anthropic, OpenAI). (S1b)

---

## 5. Context engine — `R-CTX`  *(core differentiator)*


### R-LOOP-5 — Enforced doom-loop escalation
- **Why:** upstream's `DoomLoopDetector` only nudges after the fact and cannot stop a call (principle 3).
- **What:** an identical tool call (same name, canonicalised arguments) runs normally the first time; the 1st repeat
  runs with a loud warning appended; the 2nd repeat is withheld and answered with the warning; the 3rd repeat is
  withheld and pauses the run (`InterruptionReason::DoomLoopEscalation`, exec outcome `doom_loop_escalation`, exit 6).
  Per-run counters; a one-shot re-arm lets the call through once after a pause. Orthogonal to, and does not modify,
  `hooks/doom_loop.rs` (which also catches `[A,B,C]` cycles). Behind `PEACH_HARNESS_DOOM_LOOP_ESCALATION=1`.
- **Acceptance:** unit tests of the ladder; an orchestrator spec (4 identical calls → 2 executed, 1 warned, pause);
  an `exec` run ends with exit 6; A/B before default-on.
### R-CTX-1 — Append-only event log as source of truth
- **Why:** compaction overwrites the only copy of history (S1c); Codex persists event
  history for resume/fork (S4); reversible compaction needs somewhere to recover from (S3).
- **Where:** new diesel migration in `peach_repo/src/database/migrations/`; tables
  `thread_events(conversation_id, seq, turn_id, kind, payload_json, created_at)` and
  `artifacts(hash PK, bytes, mime, size, created_at)` (content-addressed, deduped);
  domain types `peach_domain/src/thread_event.rs`; repository in
  `peach_repo/src/thread_event/`.
- **What:** every user message, assistant message (incl. reasoning), tool call, tool result
  (large payloads as artifact references), compaction event and approval is appended.
  The existing `conversations.context` becomes a **cached projection** (the working view)
  and stays for backward compatibility. Compaction writes a `compaction` event recording
  what was replaced and by what; it never deletes events.
- **Acceptance:** replaying events reproduces the pre-compaction context byte-for-byte in a
  test; resume works for old conversations without events (migration-safe); artifact
  store has a size cap and GC for artifacts unreferenced by live threads.

### R-CTX-2 — Staged compaction pipeline
- **Why:** S3 staged pattern and priority order; S2 selective retention; S1c single stage.
- **Where:** new `peach_app/src/compaction_pipeline/` (`mod.rs`, `stage.rs`,
  `supersede.rs`, `offload.rs`, `score.rs`, `summarize.rs`); invoked from
  `hooks/compaction.rs` in place of the direct `Compactor` call; existing `Compactor`
  becomes stage S3.
- **Stages, applied in order until the context is under the target budget:**
  - **S0 Supersede (deterministic, lossless w.r.t. current state):** replace results made
    stale by later actions with a stub + artifact handle — reads of a file later
    re-read in full or edited; earlier outputs of an identical command re-run later;
    search results superseded by a narrower search in the same path. Reuse the
    "last operation per file" logic from `transformers/trim_context_summary.rs`.
  - **S1 Offload (reversible):** outside the retention window, replace tool results larger
    than `offload_min_chars` (default 2,000) with a preview (first 300 chars + tool, input,
    size, exit code, error flag) and a handle. Content lives in the artifact store.
  - **S2 Score (selective, reversible):** save-token-jev algorithm (R-CTX-4) over remaining
    unpinned calls → keep / keep bounded (300-char head + handle) / drop call+result
    (still in the log; listed in the compaction note by handle).
  - **S3 Summarise (lossy, last resort):** Peach's existing summary frame, prefixed by the
    handoff note (R-CTX-6).
- Each stage must reduce by ≥ `min_stage_reduction` (default 10%) or the next stage runs;
  the pipeline stops as soon as the budget is met.
- **Invariants (tests required):** never split a call from its result; never modify user
  or assistant text in S0–S2; first user message and last `retention_window` messages
  pinned; latest reasoning signature preserved (existing behaviour); unknown content
  never removed.
- **Acceptance:** golden tests per stage; A/B on long-horizon tasks (≥ 60 turns): success
  ≥ baseline, total input tokens ↓, `recall_call` rate reported.

### R-CTX-3 — `recall` tool
- **Why:** makes S0–S2 reversible (S3); fixes save-token-jev's "just re-run it".
- **What:** `recall(handle, start_line?, end_line?, pattern?)` returns the stored content
  (shaped by R-OUT rules). Stub text always names the handle and says how to recall it.
- **Acceptance:** behaviour test where a dropped result is needed later and recalled.

### R-CTX-4 — Relevance scorer (pluggable)
- **Why:** S2.
- **Where:** `compaction_pipeline/score.rs`; trait `RelevanceScorer` with implementations:
  - `LlmScorer` — uses the configured compaction model (`compact.model`, cheap/fast).
    Port S2 exactly: pairing, pinning, state building with the degradation ladder
    (inputs ≤ 1000/200/60 chars → abridge texts → collapse old texts → one-line calls →
    drop old text-only messages), two questions per call, batching under a request budget,
    4 concurrent, threshold 0.5, missing answer = keep, malformed answer = abort stage.
    Ask the model for JSON `{id: {keep_call: p, keep_result: p}}`.
  - `HeuristicScorer` — no model: keep errors, results whose paths/identifiers appear in
    later assistant text or user messages, and results of non-idempotent commands; drop
    successful idempotent reads that were never referenced.
  - `ExternalScorer` — command hook (R-EXT-1) so third parties (e.g. save-token-jev/Jev)
    can plug in.
- **Secret redaction** before anything leaves the process (R-SAFE-3).
- **Acceptance:** unit tests with a fake scorer (like S2's test transport); decision stats
  logged per compaction.

### R-CTX-5 — Fail-open and min-reduction gate
- Scorer error, timeout (default 30 s) or reduction below 15% → skip the stage, log reason,
  continue the pipeline. The agent is never blocked by compaction. (S2)

### R-CTX-6 — Structured handoff note
- **Why:** S3 — preserve active constraints, open decisions, completed state.
- **What:** deterministic section at the top of every S3 summary: current todo list with
  statuses; user constraints (user messages containing must/never/always/don't/only or
  explicit requirements — kept verbatim); files changed so far (from `Metrics.file_operations`);
  last failing command and error; open questions. Optional LLM polish only if still over
  budget.

### R-CTX-7 — Trigger earlier, compact less often
- **Why:** S3 "deliberate, not an emergency"; prompt-cache invalidation cost.
- **What:** soft trigger at `compact.soft_threshold_percentage` (default 0.6 of the model
  window) runs S0–S1 only (cheap, deterministic); hard trigger (existing
  `token_threshold_percentage`, default 0.8) runs the full pipeline. Compact in large
  chunks so the cached prefix is invalidated rarely.

### R-CTX-8 — Cache-aware accounting
- Record provider cached-token counts per call (R-EVAL-2); report cache hit rate before and
  after each compaction; keep system prompt and tool definitions first and stable.

### R-CTX-9 — Entry-point discovery *(later)*
- **Why:** S1a FM4 (lives in proprietary Services). After the above lands: a fast first
  step that ranks likely entry files for the task using `sem_search` + ripgrep of
  identifiers from the task text, injected as a hint. Measure time-to-first-relevant-edit.

### R-CTX-10 — Scratchpad notes kept outside the context
- **Why:** S3 — a summary paraphrases or drops what the agent learned. The R-CTX-6 handoff note
  lives in the context, so the next compaction can summarise it too.
- **What:** a `write_note` tool (one flat, required `note` field) stores short notes on the
  conversation's metrics, never in `Context.messages`, so no compaction stage can touch them. When a
  compaction has removed every message showing a note, the harness re-appends all notes verbatim in
  one reminder. Notes are redacted (R-SAFE-3), capped (20 notes, 500 characters each, oldest evicted
  first), and appended to the event log (R-CTX-1), so an evicted note is still recoverable. Behind a
  flag until an A/B.

---

## 6. Harness protocol — `R-PROTO`

### R-PROTO-1 — Stable, versioned protocol crate
- **Why:** S4. **Where:** new crate `crates/peach_protocol` (types only, `serde` +
  `schemars`), new crate `crates/peach_app_server` (runtime), new subcommand
  `peach app-server`.
- JSON-RPC "lite" over stdio, one JSON object per line, request/response/notification
  shapes without the `"jsonrpc"` field. Additive evolution only; unknown fields ignored;
  `protocolVersion` negotiated in `initialize`.

### R-PROTO-2 — Translation layer, not a rewrite
- Map internal `ChatResponse` + lifecycle events to a small stable notification set. Core
  loop untouched except for exposing needed events. The UI `Notify` handshake becomes
  the server acknowledging `item/started` delivery.

### R-PROTO-3 — Primitives
- **Thread:** `thread/start`, `thread/resume`, `thread/fork` (from any turn, backed by the
  event log), `thread/archive`, `thread/list`, `thread/read`; notification `thread/started`
  (with `forkedFromId` when forked).
- **Turn:** `turn/start` (input, optional agent/model/overrides), `turn/interrupt`;
  notifications `turn/started`, `turn/completed` (with task metrics from R-EVAL-2).
- **Item** types: `userMessage`, `agentMessage`, `reasoning`, `commandExecution`,
  `fileChange` (unified diff), `toolCall` (MCP/other), `todoList`, `subagent`,
  `backgroundJob`, `compaction` (stage reached, tokens before/after). Lifecycle:
  `item/started` → `item/<type>/delta` (streaming types) → `item/completed`.

### R-PROTO-4 — Server-initiated approvals
- `item/commandExecution/requestApproval` and `item/fileChange/requestApproval` with a
  reason; the turn pauses until the client answers `allow`, `allowPrefix` (don't ask again
  for commands starting with …, persisted as a policy rule), or `deny` (the model receives
  a denial result and continues). Driven by R-SAFE-1 policy.

### R-PROTO-5 — Codegen
- `peach app-server generate-json-schema` and `generate-ts` produce client bindings from the
  Rust types; checked into `crates/peach_protocol/generated/` and verified in CI.

### R-PROTO-6 — Backward compatibility and reference client
- A tiny reference client (TypeScript, in `crates/peach_app_server/examples/`) plus
  protocol conformance tests that replay recorded sessions against the server.
- Later (not in this project's first milestone set): refactor the TUI to be a client.

### R-PROTO-7 — `exec` mode
- `peach exec "<task>" [--json]`: non-interactive (R-LOOP-4), streams protocol
  notifications as JSONL when `--json`, prints task metrics, exits 0 on completion and
  non-zero on failure/interrupt. This is what the A/B runner and CI use.

---

## 7. Safety — `R-SAFE`

### R-SAFE-1 — Ask-by-default policy
- **Where:** `peach_services/src/permissions.default.yaml`, `peach_services/src/policy.rs`.
- **Default:** allow reads anywhere in the workspace; allow writes inside the workspace;
  **ask** for writes outside it, network fetches to domains not previously approved, and
  commands matching a destructive list (`rm -rf`, `git push --force`, `git reset --hard`,
  `git clean -fd`, `sudo`, `chmod -R`, `curl … | sh`, `dd`, `mkfs`, package publish,
  database drops). `--yolo` (explicit) restores allow-all. Non-interactive mode uses a
  policy file instead of prompts and denies anything marked ask unless pre-approved.
- **Acceptance:** policy tests for every rule; approval UX works in TUI and protocol.

### R-SAFE-2 — Optional OS sandbox for shell
- Linux: Landlock (fallback bubblewrap); macOS: `sandbox-exec` profile. Write access
  limited to workspace + temp; network off unless allowed. Keep `--sandbox` worktree
  behaviour and rename the new flag `--isolate` to avoid breaking it. Probe at startup;
  fail open with a visible warning if unsupported.

### R-SAFE-3 — Secret redaction
- Redact values of keys/flags matching `api[_-]?key|access[_-]?token|token|password|secret|authorization`
  and common credential formats before sending anything to a scorer, external hook, or
  logs. (S2)

---

## 8. Prompts — `R-PROMPT`

- **R-PROMPT-1 — Measure prompt tokens per turn** by component (system prompt, each tool
  description, agent list in `task.md`). Report in R-EVAL-2.
- **R-PROMPT-2 — Compress only behind behaviour tests.** Any prompt/tool-description
  shortening requires the R-EVAL-4 suite green on both model families; prefer one
  permissive sentence over allow/deny lists (S5's fix). First candidate: `task.md`
  (long, includes example agents irrelevant at runtime).

---

## 9. Extensibility and memory — `R-EXT`, `R-MEM`

### R-EXT-1 — External hooks
- Config-declared commands receiving JSON on stdin and returning JSON on stdout, with
  timeouts and fail-open: `session_start`, `pre_tool` (may allow/deny/modify args),
  `post_tool` (may modify result), `pre_compact` (may return a replacement transcript in
  the normalized `{role, parts}` format used by save-token-jev), `post_compact`, `stop`
  (may request continuation). Map onto existing `LifecycleEvent` plumbing.
- **Acceptance:** example hooks in `examples/hooks/`; a save-token-jev-compatible adapter
  example for `pre_compact`.

### R-MEM-1 — Project memory
- `.peach/memory.md` in the workspace holding durable decisions and constraints;
  loaded at session start (bounded size); `memory_write` tool appends entries with
  source turn, requires approval in interactive mode. (S3 two-tier memory)

---

## 10. Out of scope (for now)
- Semantic response caching (D-007). Web UI. Fine-tuning. Changing Hosted Services.
  Rewriting the TUI onto the protocol (after R-PROTO lands).

## 11. Definition of done (every task)
1. Code follows `AGENTS.md` conventions; public items documented.
2. `cargo check` and `cargo insta test --accept` pass; new logic has in-file unit tests.
3. For behaviour/efficiency changes: A/B report committed under `benchmarks/reports/` with
   the decision (ship / ship behind flag / revert).
4. `TASKS.md` checkbox ticked with the report link; `DECISIONS.md` updated if a choice was made.
5. Config additions documented in `peach.schema.json` and defaults in `.peach.toml`.
