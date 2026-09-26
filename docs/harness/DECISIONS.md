# Decision log

Append-only. Format: ID, date, decision, context, alternatives, consequences. Claude Code:
when the spec is silent or ambiguous, **make the call, record it here, and continue** —
don't stop to ask unless the choice is destructive, irreversible, or changes scope.

## D-001 — Fork Peach Ice Tea rather than wrap it (2026-09-20)
- **Context:** compaction (`peach_app/src/compact.rs`), tool-output rendering
  (`operation.rs`) and the loop (`orch.rs`) are internal; Peach exposes no external hooks.
- **Alternatives:** wrapper process around the CLI; plugin via MCP.
- **Consequences:** we can change internals directly; we carry a merge burden → D-004.

## D-002 — Append-only event log; context becomes a projection (2026-09-20)
- **Context:** Peach stores one `context` blob per conversation and compaction overwrites it,
  destroying history. Reversible compaction, fork, revert and replay-based evals all need
  the original.
- **Alternatives:** keep a copy of the pre-compaction blob per compaction (simpler, but no
  fork-from-turn and quadratic storage).
- **Consequences:** new tables and a migration; artifacts content-addressed to bound storage.

## D-003 — No dependency on the proprietary Jev scorer (2026-09-20)
- **Context:** save-token-jev's algorithm is excellent but calls a hosted TypeSafe model
  with an API key.
- **Decision:** port the algorithm behind a `RelevanceScorer` trait; default LlmScorer uses
  our configured cheap model; HeuristicScorer needs no model; ExternalScorer lets Jev
  users plug in via hooks.

## D-004 — Keep upstream crate names and prefer additive modules (2026-09-20)
- **Context:** upstream ships frequently (hundreds of releases). Renames make every merge painful.
- **Decision:** new code goes into new files/modules/crates; edits to upstream files are
  minimal and marked with `// harness:` comments where non-obvious. Rebase on upstream
  at each milestone boundary.

## D-005 — Rebuild Peach Ice Tea Services capabilities in the open, selectively (2026-09-20)
- **Context:** Peach's Part 1 credits five capabilities to a proprietary Services layer
  (entry-point discovery, dynamic skill loading, tool-call correction, todo enforcement,
  reasoning-budget control). Todo enforcement exists in open source (`verify_todos`);
  the rest mostly doesn't.
- **Decision:** rebuild reasoning budget (R-LOOP-3), correction (R-TOOL-3) and entry-point
  discovery (R-CTX-9) in the open, each gated by A/B. Do not call Services APIs.

## D-006 — Protocol modelled on Codex App Server, not MCP (2026-09-20)
- **Context:** Codex found MCP semantics too thin for IDE-grade interaction (streaming,
  diffs, approvals) and built a JSON-RPC thread/turn/item protocol.
- **Decision:** same primitives and "JSON-RPC lite" over stdio JSONL; our own method
  names follow Codex's where semantics match, to ease client reuse.

## D-007 — Semantic response caching is out of scope (2026-09-20)
- **Context:** Redis pitches semantic caching for repeated questions; coding tasks rarely
  repeat verbatim and stale answers are dangerous.
- **Decision:** don't build it; rely on provider prompt caching (R-CTX-8).

## D-008 — Forked from upstream `304bf3b`; plain clone, no GitHub fork yet (2026-09-20)
- **Context:** T0.1 requires recording the upstream commit we forked from. Upstream HEAD at setup time
  was `304bf3b` (`chore(deps): update rust crate rand to v0.10.3 (#3924)`, 2026-09-20) — the same day
  the kit was written, so drift was expected to be minimal and largely was (35/36 code claims confirmed).
- **Decision:** plain `git clone` with `origin` and `upstream` both pointing at
  `https://github.com/Ankush-Jha/peach-ice-tea`. No fork created on the user's GitHub account.
- **Consequences:** nothing can be pushed until a fork exists and `origin` is repointed. Recorded as a
  guardrail in `docs/harness/DEV.md`. Clean-clone baseline verified: `cargo check --workspace
  --all-targets` clean; `cargo insta test --workspace` = 2678 passed, 1 skipped, 0 failed, no pending
  snapshots. `CLAUDE.md`'s pre-existing-upstream-failure rule therefore has nothing to record.

## D-009 — `protoc` is a build prerequisite; document it rather than vendor it (2026-09-20)
- **Context:** a clean clone fails at `peach_repo`'s build script (`tonic_prost_build::compile_protos`)
  with "Could not find `protoc`". `START_HERE.md` lists only Rust, cargo-insta, Node and Docker.
- **Alternatives:** vendor a pinned `protoc` via the `protobuf-src` crate (removes the system dependency
  but adds build time and a patch to an upstream `Cargo.toml`, against D-004).
- **Decision:** document it in `docs/harness/DEV.md` and in any CI job we add; do not patch upstream files.
- **Consequences:** one extra setup step; no merge burden. Verified with protobuf 36.2.

## D-010 — R-LOOP-4's premise is wrong: non-interactive mode already exists (2026-09-20)
- **Context:** RESEARCH.md S1c claims "No non-interactive profile found in open source." It does exist:
  `peach -p "<prompt>"` and piped stdin, gated by `Cli::is_interactive()`
  (`crates/peach_main/src/cli.rs:21,75`) and dispatched at `crates/peach_main/src/ui.rs:377`. It runs one
  turn through the same `Orchestrator::run` loop and exits, so doom-loop detection, compaction and
  `max_requests_per_turn` already apply there.
- **Decision:** re-scope T0.3 and T2.4. T0.3 adds a machine-readable output layer and outcome-aware exit
  codes to the **existing** dispatch path rather than building `peach exec` from nothing; the two failure
  reasons it must distinguish (`MaxToolFailurePerTurnLimitReached`, `MaxRequestPerTurnLimitReached`)
  already ride on `ChatResponse::Interrupt`. T2.4's remaining work is the prompt variant only.
- **Consequences:** less work than the spec assumed, and a smaller diff against upstream. Two facts to
  respect: `-p` currently always exits 0 regardless of outcome, and `--porcelain` is wired only to
  metadata subcommands, never to the chat path. Evidence in `docs/harness/RECON.md` §3.

## D-011 — Repair the eval harness before any baseline (new task T0.0) (2026-09-20)
- **Context:** 10 of the 14 evals invoke `peache --provider ... --model ...`; those flags were removed
  upstream in `b3ec4d17a` (#2685). 11 rely on `PEACH_DEBUG_REQUESTS`, which has never existed in
  `crates/`. `benchmarks/` was last touched 2026-04-10, after the removal, without being updated.
  T0.4, T0.5 and T0.6 all depend on a harness that cannot currently start the agent.
- **Alternatives:** fold the repair into T0.3 (fewer tasks, but couples a Rust change to a TypeScript
  repair and delays discovering further harness rot); or baseline only the 4 working evals (a baseline
  too narrow to gate 15 later A/Bs).
- **Decision:** add **T0.0** ahead of T0.4 — repair the invocations, standardize model naming across
  `task.yml` files, and choose a replacement for the `PEACH_DEBUG_REQUESTS` transcript-scraping
  convention. Prefer `PEACH_SESSION__PROVIDER_ID`/`PEACH_SESSION__MODEL_ID`
  (`crates/peach_config/src/reader.rs:276`) over a `peach config set` prelude step, since env vars keep
  each eval row independent and parallel-safe.
- **Consequences:** M0 grows one task. A failed baseline can no longer be misread as a regression.
  The longer-term fix is that T0.3's JSON metrics line should replace `jq`-scraping outright, so T0.0
  should avoid investing in the transcript-dump approach beyond what unblocks a baseline.

## D-012 — Correction to D-011: `PEACH_DEBUG_REQUESTS` works; T0.0 is TypeScript-only (2026-09-20)
- **Context:** D-011 and the first draft of `RECON.md` §2 stated that `PEACH_DEBUG_REQUESTS` "has never existed in
  `crates/`", inferred from `grep -rn PEACH_DEBUG_REQUESTS crates/` returning zero hits. The grep is accurate; the
  inference was wrong. `PeachConfig` has a `debug_requests: Option<PathBuf>` field
  (`peach_config/src/config.rs:173`) and `ConfigReader::read_env()` (`peach_config/src/reader.rs:104`) maps every
  `PEACH_<FIELD>` env var onto `PeachConfig` generically, with `__` as the only nesting separator — so no literal
  string exists to grep for. It is consumed by `write_debug_request` (`peach_infra/src/http.rs:238`).
- **Verified empirically** against `target/debug/peach`: `PEACH_DEBUG_REQUESTS=/tmp/probe.json peach config list`
  prints `debug_requests = "/tmp/probe.json"`; `PEACH_SESSION__PROVIDER_ID`/`PEACH_SESSION__MODEL_ID` populate
  `[session]`; `PEACH_AUTO_DUMP=json` sets `auto_dump`. Conversely `--provider` exits 2
  ("unexpected argument"), and `PEACH_OVERRIDE_PROVIDER`/`PEACH_OVERRIDE_MODEL` — used only by `todo_write_usage`,
  a third naming scheme D-011 missed — map to no `PeachConfig` field and are silently ignored.
- **Decision:** T0.0 stays in scope but shrinks to a TypeScript-only repair of `benchmarks/evals/*/task.yml`:
  replace `--provider X --model Y` and the `PEACH_OVERRIDE_*` pair with
  `PEACH_SESSION__PROVIDER_ID=X PEACH_SESSION__MODEL_ID=Y`, and **keep** `PEACH_DEBUG_REQUESTS` as-is. No Rust
  change, and no need to choose a replacement for the transcript-dump convention as D-011 assumed.
- **Consequences:** M0 is less blocked than D-011 implied — the only genuinely dead mechanisms are the two
  provider/model ones. Two caveats carry forward to T0.4 and are recorded in `RECON.md` §2: `debug_requests` is
  JSONL and captures requests only (never the final assistant message), and the evals' `jq` filters assume the
  OpenAI wire shape, so an Anthropic-native arm would silently match nothing. R-EVAL-1 requires ≥2 model families,
  so the Anthropic arm must either route through an OpenAI-compatible gateway or switch to
  `PEACH_AUTO_DUMP=json`, whose `Context` structure is provider-agnostic and also carries tool results and the
  final message.
- **Process note:** the error was reasoning from the absence of a grep hit to the absence of a feature. For a
  config value, check the config struct and its env-mapping layer before concluding it is unimplemented.

## D-013 — Shared model registry; both A/B arms route through OpenRouter (2026-09-20)
- **Context:** `R-EVAL-1` requires every A/B to run on ≥2 model families (one Anthropic, one OpenAI). Before
  this, each `task.yml` named its own models ad hoc — the same family appeared as
  `claude-sonnet-4-5-20250929`, `anthropic/claude-sonnet-4.5` and `anthropic/claude-sonnet-4.6` across
  different evals, and `create_skill` named no model at all, running against whatever ambient config existed.
  "Model family" was not usable as a dimension.
- **Decision:** `benchmarks/models.csv` (`family,provider,model`) is the single source of truth, referenced by
  every agent-invoking eval as `csv: ../../models.csv`. Sources cross-product in `benchmarks/cli.ts`, so each
  eval's task rows multiply by the model rows. Providers in `run:` lines are templated as `{{provider}}`.
- **Both arms route through `open_router`, deliberately.** The evals' `jq` validations assume the OpenAI wire
  shape (`.messages[].tool_calls[].function.name`), which OpenRouter emits. Peach's native `anthropic` provider
  sends Anthropic-shaped `content` blocks with `tool_use`, so those filters would match nothing and the eval
  would pass or fail **for the wrong reason, silently** — worse than an error, because it looks like it worked.
  One `OPENROUTER_API_KEY` also covers both arms.
- **Alternatives:** native `anthropic` + `openai` providers (needs the validations migrated to
  `PEACH_AUTO_DUMP=json` first, whose `Context` is provider-agnostic and additionally carries tool results and
  the final assistant message — the better long-term shape, deferred to T0.4); a `cmd:` source generating rows
  dynamically (**not possible** — `cli.ts:156` logs "cmd source type not yet implemented" and calls
  `process.exit(1)`).
- **Consequences:** re-baselining on a new model version is a one-line edit, but per `RECON.md` §5 it
  invalidates comparability with earlier reports, so it must be recorded here and the baseline re-run. The
  per-eval model variants upstream used to test model-specific behaviour (`z-ai/glm-4.6:exacto`,
  `minimax/minimax-m2.1`, `glm-4.7`) are dropped from the default arms; re-add them as extra registry rows if
  that coverage is wanted. Rationale is restated in `benchmarks/models.README.md` so it is not "optimised" away.

## D-014 — Two further latent breakages in the eval harness, recorded not fixed (2026-09-20)
- **Context:** found while doing T0.0; neither blocks the baseline, both would mislead later work.
- **`cmd:` sources are unimplemented.** `benchmarks/cli.ts:156-158` logs an error and hard-exits. The `Source`
  type in `benchmarks/model.ts` advertises it.
- **`type: "llm"` validations are declared but never dispatched.** `benchmarks/model.ts` defines the variant;
  `benchmarks/verification.ts::runValidations` has no branch for it, so such a validation is silently skipped
  rather than failing. The only working judge is the bespoke Vertex-AI-gated
  `benchmarks/evals/semantic_search_quality/llm_judge.ts`, invoked out-of-band via a `type: shell` validation.
- **Decision:** record both here and in `RECON.md`; do not fix under T0.0, whose scope is restoring invocation.
  `R-EVAL-3`/`R-EVAL-4` must not assume either feature exists. A silently-skipped validation is the more
  dangerous of the two and should be made a hard error when the harness is next touched (T0.4).

## D-015 — Live validation of the T0.0 repair; three findings (2026-09-21)
- **Context:** first live run against OpenRouter with a real key. The request reached the provider and
  returned `402 Payment Required` ("requested up to 20480 tokens, but can only afford 2662"), so the model
  never responded. Everything up to the model call was exercised, which was enough to confirm three things
  that had been asserted from code reading only.
- **Confirmed — `PEACH_DEBUG_REQUESTS` works (D-012).** A 52 KB `context.json` was written. Peach also
  printed "Peach no longer reads API keys from environment variables" and then "Migrated 1 provider from
  environment variables": the env var is honoured via a one-time migration into stored credentials, not by
  being read per-request. Documented in `DEV.md`.
- **Confirmed — the file is the OpenAI wire shape (D-013).** Top-level `messages`, `tools`, `model`, with
  `.messages[].role`. The evals' `jq` filters will match through OpenRouter, which is why both A/B arms
  route through it rather than the native `anthropic` provider.
- **Confirmed — `-p` exits 0 regardless of outcome (D-010).** A hard 402 with no model response still
  exited 0. Third independent confirmation; T0.3 must map outcome to exit code.

## D-016 — Eval assertions break silently once a task triggers compaction (2026-09-21)
- **Context:** `context.json` is JSONL — one appended request body per provider call — but every eval runs
  `jq -e 'FILTER' context.json`. Verified empirically: with two documents, `jq -e` reflects **only the last**
  one (a filter true for the first and false for the last exits 1).
- **Why it usually works:** each request re-sends the whole conversation, so the final document is a superset
  of the earlier ones.
- **Why it breaks:** compaction rewrites that history. Once a task is long enough to trigger it, the final
  request body carries a summary in place of the early turns, and every assertion about an early tool call —
  "did it use `patch`", "did it avoid `cat`" — silently evaluates false. The eval fails for its length, not
  its behaviour, and nothing distinguishes that from a real regression.
- **Decision:** do not paper over it in T0.0. Record it, and make migrating eval assertions to
  `PEACH_AUTO_DUMP=json` part of T0.4 — `Conversation`/`Context` is a single JSON document, provider-agnostic,
  and carries tool *results* and the final assistant message as well, none of which the request log can.
- **Consequences:** short evals are trustworthy today. Long ones are not, which makes this a prerequisite for
  T3.12's long-horizon suite (≥ 60-turn tasks — guaranteed to compact) rather than an optional cleanup.
  Until it is done, treat a failure on a long eval as unexplained until checked by hand.

## D-017 — The hackathon specification governs; no secondary models (2026-09-22)
- **Context:** `HACKATHON.md` fixes the foundation model to Gemini 3.8 High for every team and recommends restricting
  evaluation to it (§6); "unauthorized external model usage" is prohibited (§31). `SPEC.md` was written before this.
- **Decision:** where `HACKATHON.md` and `SPEC.md` conflict, `HACKATHON.md` wins. No component may call a model other
  than the configured foundation model: R-CTX-4's `LlmScorer` must use the foundation model or not exist, and the
  `HeuristicScorer` is the default. Re-prioritisation is in `ALIGNMENT.md`.

## D-018 — A/Bs and the model registry are Gemini-only (2026-09-22)
- **Context:** R-EVAL-1 required two model families; D-013 routed both arms through OpenRouter. The competition
  evaluates one model.
- **Decision:** A/Bs run against Gemini on the R-HACK-8 hackathon-shaped suite. Supersedes the two-family part of
  R-EVAL-1 and D-013's arms. `benchmarks/models.csv` moves to Gemini once a key exists; the OpenRouter rationale in
  D-013 (jq filters assume the OpenAI wire shape) stops applying once assertions move to `PEACH_AUTO_DUMP=json`, which
  becomes a prerequisite rather than a nicety.

## D-019 — Test integrity: prevent in the runtime, verify after, restore if violated (2026-09-22)
- **Context:** modifying a protected test risks disqualification (§8, §31), and test integrity is checked by file
  comparison before and after the run.
- **Decision:** refuse writes to protected paths at the tool layer, refuse obviously-mutating shell commands on them,
  tell the model which paths are protected, and hash-verify at the end. If a change still slipped through (for example
  via an unanticipated shell form), restore the protected files from the pre-run snapshot and record a loud
  `integrity` event, so the submitted state is clean and the incident is still visible in telemetry.
- **Open:** whether restoring is acceptable to the organizers (ALIGNMENT §4 Q6). Restoration sits behind a flag so it
  can be switched to report-only.

## D-020 — Telemetry and report: internal schema now, organizer adapter later (2026-09-22)
- **Context:** the organizers will supply `telemetry.schema.json` and `report.schema.json`; they are not published yet,
  and the canonical files must remain unchanged (§16).
- **Decision:** emit a rich internal event schema (R-HACK-3) and generate reports from it (R-HACK-4); isolate the
  mapping to the organizers' schemas in one adapter module per schema, so the day they are published only the adapter
  changes. Organizer files will be vendored byte-for-byte into `telemetry/` and `reporting/` and checksum-verified.

## D-021 — Submission layout without moving the workspace (2026-09-22)
- **Context:** §32 asks for `harness/`, `telemetry/`, `reporting/`, `configuration/`, `documentation/`, README, and says
  the structure may be finalised later. Moving `crates/` would break every upstream merge (D-004).
- **Decision:** add the required top-level directories; keep the Rust workspace in place; `harness/` holds the entry
  script and points at the workspace; the README documents the mapping.

## D-022 — Permissions in the evaluation profile (2026-09-22)
- **Context:** T4.1 (R-SAFE-1) plans ask-by-default approvals. In a one-shot unattended run any approval prompt blocks
  forever — a guaranteed correctness score of zero.
- **Decision:** the evaluation profile never asks. Destructive-operation safety in that profile comes from R-HACK-2's
  runtime guards and the evaluation sandbox the organizers provide, not from prompts. T4.1 stays interactive-only.

## D-023 — The harness is named "Peach Ice Tea" (2026-09-22)
- **Context:** the team named the harness. §30's evaluation record carries a harness name and version, and the name
  is what judges see in the README, the entry command, the transcript and the report.
- **Decision:** product name **Peach Ice Tea**; machine slug `peach-ice-tea`. It is used for: the README and
  `documentation/`; the entry command `harness/peach-ice-tea` (the wrapper script D-021 already places in `harness/`);
  the harness identity field in telemetry, the evidence manifest and the report (`name = "peach-ice-tea"` plus version).
  It is defined once, as a constant, so nothing hard-codes the string twice.
- **Not renamed:** the Rust crates (`peach_*`) and the `peach` binary, because renaming them breaks every upstream merge
  (D-004), and upstream's user-facing "Peach" strings (banner, prompts), which are numerous and churn with every
  release. The README states plainly that Peach Ice Tea is built on a Peach Ice Tea fork — which also answers ALIGNMENT §4
  Q1 honestly rather than obscuring it. Rebranding the binary and banner is a later, separate decision if wanted.

## D-024 — A Peach Ice Tea fork is eligible; provenance stays explicit (2026-09-23)
- **Context:** ALIGNMENT §4 Q1 was the largest unhedged risk in the project — if the organizers required a
  from-scratch harness, the whole approach would have had to change.
- **Decision (confirmed by the team, 2026-09-23):** building Peach Ice Tea on a fork of `Ankush-Jha/peach-ice-tea`
  is allowed. Work continues on the five-wave plan.
- **Consequences:** the README states the provenance plainly rather than obscuring it, and every non-trivial
  change keeps citing a requirement ID and a decision. §25's interview is about *our* engineering decisions, so
  what matters is being able to say which parts we built, why, and what evidence supports them — which is what
  `DECISIONS.md`, `RECON.md` and the review record exist to provide.

## D-025 — Total Gemini spend is capped at ₹100 (2026-09-23)
- **Context:** the team set a budget of **₹100 (~USD 1.15)** for now, which may be raised later. `CLAUDE.md`'s
  guardrail said USD 25 *per A/B run*, which is obsolete by two orders of magnitude.
- **Standing rule:** treat the current figure as a hard ceiling and ask before exceeding it, rather than
  assuming a later increase. When it is raised, the A/B plans that are written but unrun become affordable in
  priority order: TH.7 suite first, then T1.2 (line numbers) and T1.5 (noise compression), which have the
  clearest measured effect on tokens.
- **Decision:** ₹100 is a **project total**, not a per-run allowance. Consequences for the plan:
  - No A/B sweeps. R-EVAL-1's k=3 seeds × 2 arms × a suite is unaffordable, so `[A/B]` tasks stay behind
    default-off flags and unticked (already the position since D-018), and the A/B *designs* are what ship.
  - Live calls are spent only where they buy something offline testing cannot: verifying Gemini's wire
    behaviour, and a small number of end-to-end runs once the evidence pipeline is complete enough that one run
    validates many pieces at once.
  - Every live run estimates its cost first and records actual tokens from the response, so spend is tracked
    from the execution layer rather than guessed (mirroring HACKATHON §16).
  - The mock model (PLAN.md W1-D) stops being a nicety and becomes the main way end-to-end behaviour is tested.
- **Calibration:** one `gemini-3.8-flash` call at High thinking cost 260 tokens (10 prompt, 12 candidates, 238
  thinking). Thinking dominates, so output-token budget is the binding constraint, and `maxOutputTokens` matters
  more than prompt size for cost control.

## D-026 — Gemini's thinking tokens are separate from candidates, confirmed live (2026-09-23)
- **Context:** the Gemini token fix was argued from the API docs and the DTO's field structure; no live response
  was available to confirm it.
- **Evidence (live, `gemini-3.8-flash`, `thinkingLevel: HIGH`):** `promptTokenCount` 10, `candidatesTokenCount`
  12, `thoughtsTokenCount` 238, `totalTokenCount` 260 — and 10 + 12 + 238 = 260 exactly.
- **Consequence:** confirms `totalTokenCount` sums prompt + candidates + thoughts, so thinking is **not** a
  subset of candidates as it is on OpenAI. Before the fix, Peach reported 12 output tokens for 250 tokens of
  real output: about 95% of output spend invisible, on the model the harness is judged on. The fix folds
  thoughts into `completion_tokens` and keeps the raw figure in `reasoning_tokens`.
- **Also established:** `gemini-3.8-flash` exists and is reachable (1,048,576 input / 65,536 output token
  limits); there is no `gemini-3.8-pro`, so "Gemini 3.8 High" means that model at High thinking level. Note the
  65,536 output limit against `.peach.toml`'s `max_tokens = 20480`, which counts thinking and would truncate
  High-thinking turns — tracked for TH.3.

## D-027 — Relevance-scored compaction: pluggable scorer, Gemini in the evaluation profile (2026-09-23)
- **Context:** the team asked for Jev-style relevance scoring to drive compaction decisions, and said to use
  whatever models make the product strong, going beyond what the hackathon strictly demands. `HACKATHON.md` §31
  prohibits "unauthorized external model usage" *during the evaluation run*, and §6 recommends restricting
  evaluation to the standardized model. D-003 had previously refused any Jev dependency outright.
- **Decision:** build the capability fully and keep the frozen run compliant. `RelevanceScorer` is a trait with
  three implementations:
  - `HeuristicScorer` — no model at all; recency, pinning and result size. The fallback when anything else
    errors or times out.
  - `LlmScorer` — the ported Jev algorithm, asking the **configured foundation model** (Gemini) the two
    probability questions per tool call. **This is the default, and the only one enabled in the evaluation
    profile.**
  - `JevScorer` — the hosted TypeSafe scorer, behind config and off by default, for use outside a judged run.
  This supersedes D-003's blanket refusal: the algorithm is ported rather than depended on, and the hosted
  scorer becomes an option instead of a requirement.
- **Why the evaluation profile is pinned to Gemini:** a second external model inside the judged run is exactly
  what §31 names, and the penalty is disqualification. Losing on a technicality would defeat the point of a
  stronger product, so capability and compliance are separated by configuration rather than by argument.
- **Scope (team's call):** compaction only — which tool calls and results to keep verbatim, truncate or drop
  (`R-CTX-4`). Not what enters context, and not tool routing.
- **Consequence:** `R-SAFE-3` redaction (T3.7) moves ahead of the scorer work, because tool previews leave the
  process the moment any scorer is called. T3.7, T3.8 and T3.9 move from Tier 3 to Tier 2 in `ALIGNMENT.md`.
