# Research digest — the evidence behind every requirement

This file is the source of truth for *why* the harness is built the way it is.
Every requirement in `SPEC.md` carries an ID (e.g. `R-OUT-2`), and every ID is
traced back to a finding here. If you (Claude Code) are about to deviate from a
requirement, find its evidence here first and write down in `DECISIONS.md` why
the evidence doesn't apply.

All findings are paraphrased. Numbers are as reported by each source for its own
workload; **they are hypotheses for us, not guarantees** — we re-measure
everything (see `R-EVAL-*`).

Sources studied (Sept 2026):

| Key | Source | What it is |
|---|---|---|
| S1 | forgecode.dev home page | Product claims, benchmark position |
| S1a | ForgeCode blog, *Benchmarks Don't Matter* Part 1 (Mar 2026) | 7 failure modes, 25% → 78.4% on TermBench 2.0 |
| S1b | ForgeCode blog, Part 2 (Mar 2026) | 78.4% → 81.8% with GPT 5.4 and Opus 4.6 |
| S1c | `tailcallhq/forgecode` source, main branch (inspected 20 Sep 2026) | What is actually implemented |
| S2 | `IamUnbounded/save-token-jev-clean` README + `src/core.ts` | Selective, relevance-scored compaction |
| S3 | Redis blog, *Context compaction for AI agents* (May 2026) | Compaction taxonomy, reversible vs lossy |
| S4 | OpenAI, *Unlocking the Codex harness* (Feb 2026) + app-server README | Harness as a server; protocol primitives |
| S5 | GitHub blog, *How we make AI coding more cost efficient…* (Sep 2026) | Four shipped efficiency changes, evaluation discipline |

---

## S1 / S1a / S1b — ForgeCode: what made it #1

### Product claims (S1)
- Claims #1 on TermBench 2.0 at 81.8%, ahead of Warp (61.2%), Claude Code (58%), OpenCode (51.7%) in their chart.
- ZSH-native: `:` prefix talks to the agent inside the user's shell; aliases and plugins keep working.
- Multi-agent with bounded context: FORGE (execute), MUSE (plan), SAGE (research).
- Model mixing mid-session across many providers.
- Claims "thousands of evaluations" per change before shipping.
- **Implication:** our baseline is already the strongest open harness on task completion. We must not regress it. → `R-EVAL-1`, `R-EVAL-2`.

### Part 1 — seven failure modes (S1a)
1. **Interactive-by-default kills autonomous runs.** The agent asked clarifying questions nobody would answer. Fix: a separate non-interactive runtime profile (no clarification, assume defaults, commit to an answer). → `R-LOOP-4`
2. **Good descriptions ≠ correct tool use.** Failures split into wrong tool, wrong argument names, wrong sequencing. These are invisible in aggregate pass rate; they built per-tool, per-model micro-evals. → `R-EVAL-3`
3. **Naming is a reliability variable.** Renaming edit-tool args to `old_string`/`new_string` (names common in training data) measurably reduced errors. → `R-TOOL-2`
4. **Context helps only after the right entry point is found.** Bottleneck is entry-point discovery latency, not token count. They built semantic entry-point discovery — **in the proprietary ForgeCode Services layer**. → `R-CTX-9` (future), noted as gap.
5. **Time limits punish meandering trajectories.** Every failed call and redundant read burns wall-clock budget; timeout = failure. → `R-LOOP-1`, `R-LOOP-2`
6. **Planning tools only work when enforced.** `todo_write` enforcement took pass rate 38% → 66%. → keep; `R-EVAL-4`
7. **Speed beats intelligence under time limits.** Two structural changes: low-thinking subagents for easy parallel work, and a *progressive thinking policy*: very high reasoning for the first 10 assistant messages, low afterwards, high again whenever verification runs. → `R-LOOP-3`

- Trajectory: ~25% baseline → ~38% (non-interactive + naming + micro-evals) → 66% (todo enforcement) → 78.4% (subagents + progressive thinking + skill routing).
- **Critical finding:** Part 1 says five capabilities live in **ForgeCode Services, a proprietary layer**: semantic entry-point discovery, dynamic skill loading, a tool-call correction layer (heuristics + static analysis before dispatch), `todo_write` enforcement, and reasoning-budget control. **A fork of the open-source repo does not automatically get the 81.8% configuration.** We must rebuild the parts we need in the open. → `R-LOOP-3`, `R-TOOL-3`, `D-005`
- They gate releases in CI on: per-tool correctness per model, todo compliance, entry-point precision, skill-routing accuracy.
- Their stated next measurements: per-tool reliability by model, entry-point latency distribution, **recovery rate after the first tool error**, time-efficiency under tight budgets, cross-model variance. → `R-EVAL-2`

### Part 2 — model-specific failure compensation (S1b)
- Remaining failures: argument typos, nested-schema confusion, truncation blindness, premature completion.
- **Fix 1:** put `required` before `properties` in every schema — fewer malformed calls from GPT 5.4. Adopted schema-wide. → `R-TOOL-1` (keep)
- **Fix 2:** flatten nested schemas — one `required` array, one object layer. → `R-TOOL-1` (keep)
- **Fix 3:** make truncation impossible to miss — put a plain-text "N more lines truncated, call read again with a range" line in the result body; metadata alone was ignored by GPT 5.4. → `R-OUT-4`
- **Fix 4 (largest single gain):** a verification skill switches the model to reviewer mode (what was asked, what was done, what evidence, what's missing), **enforced by the runtime** — prompting "please verify" did nothing. → keep; `R-EVAL-4`
- Opus inferred truncation and self-verified; GPT needed it explicit. Same score after the harness compensated.
- Their stated next work: per-tool reliability tracking by model, schema-shape evals before new tools ship, when to enforce vs skip verification, keep-going-vs-stop analysis, provider-specific runtime defaults. → `R-TOOL-4`, `R-EVAL-3`

### What the open-source code actually does (S1c) — verified by reading source
Paths are relative to `crates/`.

**Loop and hooks**
- `forge_app/src/orch.rs::Orchestrator::run` — the loop. Ends when the model stops with no tool calls, a yield tool is called, tool errors exceed `max_tool_failure_per_turn` (3), or `max_requests_per_turn` (100) is hit. The `on_end` hook may add messages and continue the loop.
- `orch.rs::execute_tool_calls` — **`task` (subagent) calls run in parallel via `join_all`; every other tool call runs sequentially**, even though `shell.md` tells the model to issue independent calls in parallel. → `R-LOOP-1`
- `forge_app/src/app.rs` wires hooks: `on_request` = tracing + `DoomLoopDetector`; `on_response` = tracing + `CompactionHandler`; `on_end` = tracing + title + `PendingTodosHandler` (if `verify_todos`).
- `forge_domain/src/hook.rs` — `LifecycleEvent`: start, end, request, response, toolcall start, toolcall end. **In-process Rust traits only; no user-configurable external hooks.** → `R-EXT-1`
- `forge_app/src/hooks/doom_loop.rs` — detects ≥3 identical consecutive calls or a repeating call pattern; injects `templates/forge-doom-loop-reminder.md`.
- `tool_timeout_secs = 300` in `forge_config/.forge.toml`: shell blocks up to 5 minutes. **No background execution anywhere** (searched all crates). → `R-LOOP-2`
- Reasoning effort is static per agent/config (`forge_app/src/agent.rs`). **No progressive thinking policy in open source.** → `R-LOOP-3`
- ~~No non-interactive profile found in open source.~~ **Corrected 2026-09-20 (D-010): this is wrong.** `forge -p "<prompt>"` and piped stdin already exist (`forge_main/src/cli.rs:21`, `Cli::is_interactive()` at `:75`, dispatched at `forge_main/src/ui.rs:377`) and run one turn through the same `Orchestrator::run` loop before exiting. The real gaps are that its output is terminal-formatted rather than machine-readable (`--porcelain` is wired only to metadata subcommands, never to the chat path) and that it always exits 0 regardless of task outcome. → `R-LOOP-4` is re-scoped to the prompt variant only; see `RECON.md` §3.

**Tool output**
- `forge_app/src/operation.rs::into_tool_output` renders every tool result as XML-ish elements.
- File read: up to 2,000 lines, 2,000 chars per line; `show_line_numbers` **defaults to true** (`forge_domain/src/tools/catalog.rs`, `FSRead`, `default_true`); `fs_read.md` says results use `rg -n` format. → `R-OUT-1`
- Edits are **exact string replacement** (`fs_patch.md`), yet the description spends a paragraph telling the model to strip the line-number prefix. Line numbers are vestigial for editing — the exact situation GitHub fixed (S5). → `R-OUT-1`
- Shell: `truncation/truncate_shell.rs` keeps the first 100 and last 100 lines, 500 chars per line — **the same for every command** (`git diff` is clipped like an `npm install` log). Full stdout/stderr go to temp files via `tool_executor.rs::dump_operation` and the path is shown. → `R-OUT-2`, `R-OUT-3`
- Fetch: 50,000 chars with path to the rest. Search: line and byte caps with a "use a more specific pattern" message.
- `forge_domain/src/session_metrics.rs::Metrics` tracks start time, file operations, files accessed, todos — **no task-level cost fields (tokens, cached tokens, LLM calls, wall time, recovery events).** Usage is accumulated on message entries but not reported per task. → `R-EVAL-2`

**Compaction**
- Trigger: `forge_domain/src/compact/compact_config.rs::should_compact` — any of tokens ≥ `token_threshold` (100k default, also derived from model window), user turns ≥ `turn_threshold`, messages ≥ `message_threshold` (200), or on turn end. Checked in `CompactionHandler` on every response.
- Range: `compact/strategy.rs` — `min(evict 20% of tokens, retain last 6 messages)`; starts at the first assistant message; never splits a tool call from its result.
- `forge_app/src/compact.rs::Compactor::compress_single_sequence` builds a `ContextSummary` and renders `templates/forge-partial-summary-frame.md`: **user/assistant text kept verbatim; each tool call becomes a one-line header (`Read: path`, `Execute: cmd`, `Update: path`, todo diffs); every tool result is discarded.** No LLM call.
- `transformers/compaction.rs::SummaryTransformer`: drop system messages, dedupe consecutive user messages, keep only the last operation per file path, strip the working directory.
- Preserves the latest reasoning signature by injecting it into the first surviving assistant message (needed for extended-thinking chains).
- **Persistence:** `forge_repo/src/database/schema.rs` has one `conversations` row with a single `context` text column. `app.rs::compact_conversation` replaces `conversation.context` with the compacted context and persists it. **Compaction overwrites history. There is no event log, no way to recover dropped tool output, no fork, no replay.** → `R-CTX-1`

**Safety**
- `forge_services/src/permissions.default.yaml`: allow `read **/*`, `write **/*`, `command *`, `url *`. **Everything allowed by default.** → `R-SAFE-1`
- `--sandbox <name>` (`forge_main/src/sandbox.rs`) creates a **git worktree**; no process, filesystem or network isolation. → `R-SAFE-2`

**Surfaces**
- ZSH plugin, TUI, one-shot CLI. `ChatResponse` (`forge_domain/src/chat_response.rs`) is the internal event stream: `TaskMessage`, `TaskReasoning`, `TaskComplete`, `ToolCallStart` (with a `Notify` handshake so the UI renders the header first), `ToolCallEnd`, `RetryAttempt`, `Interrupt`. **No external protocol.** → `R-PROTO-*`

**Evals**
- **Correction 2026-09-20 (D-011): 10 of these 14 evals cannot currently invoke the binary at all.** They pass `--provider`/`--model`, removed upstream in `b3ec4d17a` (#2685), and 11 rely on a `FORGE_DEBUG_REQUESTS` transcript-dump env var that has zero occurrences in `crates/`. `benchmarks/` also runs in no CI workflow. A baseline run today reports ~0% pass — a broken harness, not a regression. See `RECON.md` §2 and task T0.0.
- `benchmarks/evals/` has 14 behavioral evals: commit_no_markdown, create_skill, echo, multi_file_patch, parallel_tool_calls, patch_exact_match, read_over_cat, redundant_cd_with_cwd, refactoring_uses_patch, search_over_find, sem_search, semantic_search_quality, suggest, todo_write_usage. They measure behavior and success — **not cost per completed task.** → `R-EVAL-2`

**Conventions to obey** — from the repo's `AGENTS.md`: tests in the same file, `pretty_assertions`, fixture/actual/expected naming, `derive_setters` with `strip_option` + `into`, `///` docs on every public item without code examples, services never depend on other services, one generic infra parameter stored as `Arc<T>`, no `Box<dyn …>` in services, verify with `cargo check` and `cargo insta test --accept`, never `cargo build --release` for verification.

---

## S2 — save-token-jev: relevance-scored compaction

**Idea.** Don't ask an LLM to rewrite history into a lossy summary. Ask a scorer which *tool calls and results* still matter. User and assistant text stay verbatim. Each call is kept with its full result, kept with a bounded result, or removed together with its result.

**Algorithm (`src/core.ts`), step by step:**
1. `collectToolCalls`: pair every `tool_call` with its `tool_result` by call ID. A call is **pinned** if it or its result is the first message or within the newest `preserveRecentMessages` (6).
2. `fitState`: build a compact "state" for the scorer — the goal (last 3 user messages, 500 chars each, unless given) plus history where every tool result is replaced by `ok|error, N chars (omitted)`. If the state exceeds `maxStateTokens` (25k), degrade in order: cap call inputs at 1,000 → 200 → 60 chars; abridge long texts to head 400 + tail 150; collapse old unpinned texts to "[N chars omitted]"; compact old calls to one-liners; finally drop old text-only messages. Fail if it still doesn't fit.
3. `questionsFor`: two probability questions per unpinned call — *does knowing this call ran still matter?* and *must the full result stay verbatim because re-running wouldn't recover it?*
4. `batchCalls` + `askBatches`: pack questions into requests under `maxRequestTokens` (30k), 4 concurrent.
5. `decide` (threshold 0.5): pinned → keep; keepResult ≥ t → keep; keepCall ≥ t → truncate result to a 300-char head plus a note; else drop call and result. Missing answers default to keep.
6. `applyDecisions` removes/truncates parts; empty messages disappear.
7. Stats: chars before/after, counts per decision, scorer stage, requests, time.

**Safety properties:** results always removed together with their call; first and newest messages pinned; text and unknown ("opaque") blocks never touched; malformed or incomplete scorer answers fail the whole attempt; integrations **fail open** to the host's native compaction; takeover only if reduction ≥ 15%; secrets (`api_key`, `token`, `password`, `secret`, `authorization`) redacted from input previews sent to the scorer.

**Host integration lessons:** Codex has no hook that can replace the compacted transcript, so the plugin uses `PreCompact` to compute and `SessionStart(source=compact)` to re-inject. OpenCode and Claude Code expose hooks that replace the result directly. **Forge has no hook at all, which is why it isn't a supported host.**

**Limitations we must fix, not copy:**
- The scorer is a proprietary hosted model (TypeSafe "Jev", API key required). We will not depend on it. → `D-003`
- Truncated/dropped results tell the model to "rerun the tool if needed" — not reversible for non-idempotent commands. We offload to a store instead. → `R-CTX-3`
- Token counts are estimates.

→ `R-CTX-4`, `R-CTX-5`, `R-EXT-1`

---

## S3 — Redis: compaction taxonomy

- **Why:** without caching, every token in the window is re-processed and billed each call; cost per turn grows with session length; latency rises and recall of early information degrades. Bigger windows don't remove the need; RAG doesn't manage the agent's own working state.
- **Definition:** condense a near-full conversation into a structured, high-fidelity representation and continue — like an engineering handoff note, not a Slack export.
- **Preserve:** active constraints still in force, open decisions not yet acted on, completed-task state. **Discard:** exploratory process, tool outputs already reflected in agent state, verbose intermediate steps. → `R-CTX-6`
- **Not truncation** (cuts blindly at a boundary) and **not plain summarisation** (prose that may lose exact numbers/phrasing whose importance appears later).
- **Priority order:** raw context → *reversible* compaction (dropped content still exists elsewhere and can be fetched) → *lossy* summarisation only when nothing cheaper works. → `R-CTX-2`
- **Patterns:** sliding windows (bounded but forgetful); token-threshold + LLM summary (easy but uncontrolled and nondeterministic); **tool-output offloading** (store externally, leave pointer + preview; nothing destroyed); **staged compaction** (mask unused fields, prune stale turns, summarise last). → `R-CTX-2`, `R-CTX-3`
- Skip lossy compaction where exact wording matters (legal text, precise API responses).
- Compaction should be a deliberate architectural choice, **not an emergency when the window is already full**. → `R-CTX-7`
- A *context engine* decides per step what enters the window: selection, compression, retrieval, routing. Two memory tiers: session-scoped working memory, and long-term cross-session memory. → `R-MEM-1`
- Freshness: a perfect summary of stale state still misleads.
- Semantic caching (LangCache) is pitched for repeated questions — **low value for a coding agent; out of scope.** → `D-007`

---

## S4 — Codex: the harness as a server

- One harness (agent loop + logic) powers web, CLI, IDE extension and desktop app through the **App Server**, a bidirectional JSON-RPC API.
- History: tried Codex-as-MCP-server first; MCP semantics didn't fit rich IDE interaction (streaming, diffs). Built a JSON-RPC protocol mirroring the TUI loop, then hardened it into a backward-compatible platform after JetBrains, Xcode and the desktop app needed it. → `R-PROTO-1`
- **Harness contents beyond the loop:** (1) thread lifecycle and persistence — create, resume, fork, archive; event history persisted so clients reconnect to a consistent timeline; (2) config and auth; (3) tool execution in a sandbox, plus MCP and skills under one policy model. → `R-PROTO-3`, `R-SAFE-2`
- `codex core` is both the agent library and a runtime managing one thread's persistence.
- **App Server process:** stdio reader → message processor → thread manager → one core session per thread. The processor translates client requests into core operations and turns core's low-level internal events into a **small set of stable, UI-ready notifications**. → `R-PROTO-2`
- **Bidirectional:** one client request → many server notifications; the server can also send requests (e.g. approval) that pause the turn until the client answers. → `R-PROTO-4`
- **Primitives:**
  - *Item* — atomic typed unit (user message, agent message, tool execution, approval request, diff) with lifecycle `item/started` → optional `item/*/delta` → `item/completed`.
  - *Turn* — one unit of work started by user input; contains items.
  - *Thread* — durable container of turns; create/resume/fork/archive; persisted history.
- **Handshake:** client must send `initialize` (with `clientInfo`) first; server responds and advertises capabilities; both agree on version, feature flags, defaults.
- **Flow:** `thread/start` → `turn/start` → `thread/started`, `turn/started`, `item/started`/`item/completed` for the user message → tool items; `item/commandExecution/requestApproval` with a reason → client allow/deny (VS Code offers "yes", "yes and don't ask again for commands starting with X", "no") → `item/*/delta` streaming agent text → `item/completed` → `turn/completed`.
- **Transport:** JSON-RPC "lite" — no `"jsonrpc":"2.0"` header, JSONL over stdio. Remote/hosted setups tunnel stdio over a persistent connection.
- **Codegen:** `generate-ts` and `generate-json-schema` from the Rust protocol types so clients in any language can bind. → `R-PROTO-5`
- **Clients:** local apps bundle a pinned server binary as a long-lived child process; some partners decouple release cycles by pointing a stable client at a newer server — **so the protocol must stay backward compatible**. Web runs the server in a container; server holds state so work survives closed tabs. TUI is being refactored to be just another client, enabling remote servers that keep working when the laptop sleeps. → `R-PROTO-6`
- **Other embeddings:** MCP server mode (limited semantics), `exec` (non-interactive, structured output, clear exit code for CI), SDK. → `R-PROTO-7`
- Later README items: `thread/revert` replaced rollback; `rollout/compress`; stored thread attachments copied on fork.

---

## S5 — GitHub Copilot: efficiency without quality loss

**Principle:** optimise the outcome, not the tool call. A concise tool response can cost more overall if it omits something the agent then has to fetch.

**Method:** offline agentic benchmarks first, then controlled online A/B experiments; the same harness serves Copilot CLI, the Copilot app and code review, so wins propagate. → `R-EVAL-1`

**Local metric trap:** RTK (a shell-output shortener) made individual outputs shorter, but the agent re-read or re-ran commands to recover missing detail; **total tokens and duration went up.** Evaluate across the whole task.

**Change 1 — selective output compression (~5.5% cost reduction in their chart):**
- Install/build/test/lint output is mostly repetitive noise; source-like and arbitrary command output usually contains what the agent needs.
- Early versions were too aggressive (they even compressed `git diff` and had to undo it).
- Final policy: (1) return source-like and arbitrary output unchanged (`cat`, `git diff`, `git show`, scripts); (2) reorganise search results without dropping any match; (3) compress repetitive noise only when savings are substantial.
- The full original is always retrievable. **Recovery path doubles as a metric**: they tracked opening the saved original, re-running commands, repeated exploration, narrower searches, extra turns. Recovery was extremely rare; no significant task-success regression. → `R-OUT-2`, `R-OUT-3`

**Change 2 — remove line-number prefixes from file views:**
- Prefixes existed for old line-number-based edit tools; current tools match surrounding text.
- ~5% lower inference cost offline; ~3% lower average daily cost per user online; success within variance; edit failures did not increase.
- Line numbers stay useful in diffs and short snippets. → `R-OUT-1`

**Change 3 — compress prompts without compressing intent (~2.9%):**
- A meta-prompting loop halved the `task`-tool guidance; targeted behavior tests checked it.
- Online, it regressed: cautious parallelism guidance became a hard rule and **custom agents ran sequentially**. They stopped, wrote a regression eval for that behavior, and replaced an allow/deny list with one sentence saying independent agents can run in parallel with attention to side effects.
- Result: ~1,300 fewer prompt tokens per turn, ~1.8% fewer prompt tokens per session, ~2.9% lower cost per active hour. → `R-PROMPT-1`, `R-PROMPT-2`

**Change 4 — deliver completed background work without a retrieval turn (~2.3%):**
- Background shell commands and subagents notify the model when done; before, the notification lacked the result, costing an extra turn per completion.
- Now eligible completions are **batched and delivered in the existing tool-result format**; one model call processes both results where four were needed. Explicit reads of still-running work are unchanged. → `R-LOOP-2`

**Measure in context:** a tighter file-tool instruction set that helped code review *increased* cost in CLI, so it didn't ship. Line-number removal and selective compression each cut prompt tokens per review ~5% in code review. Separately, moving code review to shared file tools plus instruction tuning cut its cost ~20%. → `R-EVAL-1`

**Five lessons:** optimise the completed task; optimise orchestration, not just output; compress by what the output represents, prefer lossless, measure recovery; prompt rewrites have side effects — test behavior; evidence is local to the workload.

---

## Cross-source synthesis — what "best in class" means here

| Dimension | Best evidence | Forge today | Our target |
|---|---|---|---|
| Tool-call reliability | S1a, S1b | Strong | Keep; add per-tool/model tracking and pre-dispatch correction |
| Verification / planning enforcement | S1a, S1b | Strong | Keep; add eval gates |
| Output shaping | S5 | Uniform head/tail, line numbers on | Content-aware, lossless-first, recovery-tracked |
| Orchestration | S5, S1a | Subagents parallel, tools serial, no background | Parallel read-only tools, background jobs, batched delivery, progressive reasoning |
| Compaction | S2, S3 | One-shot, deterministic, destroys results and history | Event log + staged, reversible, relevance-scored, cache-aware |
| Persistence | S4 | Single overwritten blob | Append-only event log; resume/fork/revert |
| Protocol | S4 | None | Thread/turn/item JSON-RPC server, approvals, codegen, `exec` |
| Safety | S4 | Allow-all, worktree only | Ask-by-default for risky actions, OS sandbox option, secret redaction |
| Extensibility | S2, S4 | In-process only | External JSON hooks incl. pre/post compaction |
| Evaluation | S5, S1a | Behavior + pass rate | Cost per completed task, recovery rate, paired A/B, behavioral regression suite |
