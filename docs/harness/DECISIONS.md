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

## D-028 — The test-integrity guard is wired but INERT; activating it is the next task (2026-09-23)
- **Context:** TH.2 shipped the guard and wired it into tool dispatch, and an earlier summary of mine described it
  as "enforced at runtime". A worker on TH.1 grepped for callers of `peach_harness::runtime::install` and found
  none anywhere in the codebase.
- **Correction:** the dispatch-layer check is real, but every accessor answers "not active" when no runtime is
  installed — which is deliberate, so interactive use is unaffected — and nothing installs one. **The guard
  currently protects nothing.** A model could edit a protected test today and only the post-run hash comparison
  in the eval runner would notice.
- **Next action when work resumes:** install a `HarnessRuntime` at the start of an `exec` run with the repository
  root, the resolved protected set, and the non-interactive flag, then verify end to end with the eval suite's
  `cheat` agent driving the real binary rather than a stub.
- **Related hand-backs, not yet applied:** `peach_services` needs a `peach_harness` dependency and a two-line
  guard in `followup.rs` so the model cannot ask a question mid-run (the runtime side is done); the same guard
  belongs in the MCP trust gate if MCP init is ever added to `exec`; and `peach_domain::TaskOutcome` should gain
  a `TimeBudget` variant so the time-budget report stops being hand-built JSON.

## D-029 — An unbounded `exec` can stall on provider retries (2026-09-23)
- **Context:** while building the wall-clock budget, a worker pointed a configured provider at a closed local
  port. The retry policy backed off and kept going for **2 minutes 10 seconds** before being killed by hand — no
  prompt, no hang, just retries against an unreachable endpoint.
- **Consequence:** in a frozen one-shot run, that alone could consume the judging window, and it would look like
  the harness "did nothing" rather than like a network fault.
- **Decision:** `--max-duration-secs` defaults to off, because changing default behaviour needs evidence we
  cannot gather without budget (D-025). The **frozen evaluation invocation must always pass it**, and the
  `harness/peach-ice-tea` wrapper (D-021/D-023) is where that belongs. Exit code 4 and outcome `time_budget`
  distinguish it from a task failure, so the evidence says which happened.

## D-030 — D-028 closed: the guard is live, verified, and proven against the real binary (2026-09-25)
- **Context:** D-028 part 1 installed the runtime. Wiring the post-run check turned up another gap: part 1
  captured the manifest with **no snapshot directory** and then dropped it, so a violation could have been
  detected but never restored. Separately, nothing anywhere called `telemetry::install`, so telemetry events had
  no sink.
- **Decisions:**
  - The snapshot goes in a system temp dir that lives as long as the run, which keeps it outside the repo by
    construction. `ExecHarness::finish` verifies and restores on all three exit paths (success, error, time budget).
  - `TaskOutcome::TimeBudget` (exit 4) replaces the hand-built timeout JSON. This was the D-028 hand-back.
  - `ExecReport.integrity` carries `{checked, violations[{path, kind, restored}]}`. It does not change the exit
    code (PLAN.md C5).
  - **`exec --telemetry <FILE>`** is an interim flag. TH.5's `--evidence-dir` should default it to
    `<dir>/telemetry.jsonl` rather than replace it, so the eval runner's sniffing keeps working.
  - The telemetry stream now carries `run_start`, `integrity` (`refused` / `verify` / `restored` / `violation`) and
    `run_end`. **`model_call`/`tool_call` events are still not emitted:** the TH.4 hook into peach_app hooks
    was never wired.
- **Proof, not assertion:** the old `--agent cheat` stub never ran peach, so it only proved the *runner's* check.
  The new `run.ts --agent peach-cheat` runs the real binary against a closed-port provider (no model, no spend). It
  waits for the harness's `run_start`, overwrites a declared test, and adds a short-circuiting new test mid-run.
  It passes only if peach's own JSON reports both as restored **and** the runner's independent hash + git-diff
  check finds the tree clean. Result: 3/3 fixtures. The same scenario is a cargo test
  (`peach_main/tests/exec_integrity.rs`).
- **Still not proven end to end:** the *dispatch-time refusal*. That needs a model that actually tries to edit a
  test, which means the mock model (PLAN.md W1-D step 4, never built) or a live run.

## D-031 — `followup` can no longer end or block a one-shot run (2026-09-25)
- **Context:** the D-028 hand-back was to gate `peach_services`' `followup.rs` so an unattended run answers
  instead of prompting. Reading the orchestrator showed this alone would not help: `orch.rs` sets `should_yield`
  from the tool **name** before any tool runs, so every `followup` call ended the turn. In `exec` that ends the
  run, reported as `completed`, mid-task. That includes a *hallucinated* call from the default `peach` agent,
  which has no `followup` tool at all. That second case is the likelier one in a judged run. It was reproduced
  with a scripted model: 1 model call, exit 0, task unfinished.
- **Decision:** both halves go through `peach_harness::runtime`, active only when non-interactive:
  - `followup.rs` returns `UNATTENDED_FOLLOWUP_ANSWER` ("No human is available… proceed on your own judgement,
    state the assumption") and emits `prompt_suppressed`;
  - `orch.rs` yields on `followup` only when `followup_ends_turn()`.
  Interactive behaviour is unchanged. This is a Tier 1 correctness precondition (PLAN.md C2), so it ships on in
  `exec` without an A/B.
- **Proof:** `peach_main/tests/exec_scripted_model.rs` runs the real binary against a local scripted
  OpenAI-compatible endpoint (`tiny_http`, no spend). It covers a hallucinated followup (the loop continues),
  a custom agent that has `followup` (the model receives the unattended answer), and a `write` to a protected
  test (refused at dispatch, file unchanged, refusal visible to the model, `integrity:refused` in telemetry).
  The last one closes the gap D-030 left open. Reverting the `orch.rs` line makes the first test fail.
- **Found on the way:** `ProtectedSet::display_path` left a path absolute when it reached the repo through a
  symlink (macOS `/var` → `/private/var`). This made refusal text and telemetry show absolute paths. Worse,
  when an ancestor directory matched an exclude glob (a repo under `build/`), a real test was treated as
  unprotected. It now falls back to canonical paths, including for files that don't exist yet. A regression
  test fails on the old code.
- **Flaky upstream test, not touched:** `peach_repo … test_concurrent_operations_dont_block_runtime` failed once in
  a full parallel run (2879/2880). It passed 3/3 in isolation and on the next full run (2880/2880). It asserts
  on 10 ms ticks inside a 200 ms window, so it is load-sensitive. Recorded per CLAUDE.md; not weakened.

## D-032 — First live end-to-end run on Gemini 3.8 Flash: fixed, but stopped by my cap (2026-09-25)
- **Setup:** `run.ts --agent peach --suite py-bugfix` against the real Google endpoint, `gemini-3.8-flash`
  at High. The id is registered through an isolated `PEACH_CONFIG`, since the built-in list lacks it. Caps:
  `PEACH_MAX_TOKENS=8192`, `PEACH_MAX_REQUESTS_PER_TURN=12`, `--max-duration-secs 480`.
- **Pre-run estimate (D-025):** ~₹15 expected, ≤₹56 for the main agent. It was based on a free dry run
  against a closed port, which measured 12.7k input tokens per request. That dry run also found TH.3's
  thinking-level mapping unwired (fixed in `20500d10f`).
- **Result:** the source fix was correct. The tests pass, and peach's own integrity check agrees with the
  runner's independent one (clean, 2 checked). The outcome was still **`request_limit` (exit 3)**: the model
  spent its 12 requests fixing, verifying, and then answering the todo-enforcement reminder. **The cap was mine,
  set for cost**, and the evaluation default is 100. This is not evidence of a harness failure, but it is not a
  success either.
- **Actual spend (execution layer):** 13 metered calls, 239,570 input tokens (125,924 cached), 9,164 output
  (8,296 of them thinking), ≈ **USD 0.13 ≈ ₹11** at USD 0.75/3.75 per 1M (in/out) and 0.075 cached.
  Upper bound **≈ ₹22**: the 3 empty completions below are not metered, and each may have been billed.
  Budget remaining ≥ ₹77.
- **Findings, all real and all offline-fixable:**
  1. **17 silent retries:** 11 × HTTP 429, 3 × 503, 3 × "Empty completion received". The wire saw 31 requests
     against 13 counted calls. `failed_llm_calls` stayed 0 and no `retry` event exists: R-HACK-3 / §15 require
     retries in telemetry, and cost accounting misses billed-but-empty responses. Retry backoff was ~40% of the
     208 s wall time.
  2. **Tool schemas are 75% of every request** (38 KB of 50.8 KB). That is the largest fixed token cost,
     and it matters for §19.
  3. **Telemetry carried only `run_start` / `integrity` / `run_end`.** TH.4's model/tool hooks are not wired,
     so this run's per-call evidence exists only in `PEACH_DEBUG_REQUESTS` and the peach log.
  4. Gemini accepted `thinkingLevel: "high"` (lowercase). There were no 400s.
- **Next live run** should wait until retries are metered and TH.4's model_call/tool_call events are wired,
  and should use the default request limit (or ≥ 30).

## D-033 — TH.4 wired: model, tool, agent-state and retry telemetry; retries metered (2026-09-25)
- **Context:** D-032's live run showed 31 requests on the wire against 13 counted calls. That was 17 retries
  (11 × 429, 3 × 503, 3 × empty completion) with nothing in the metrics or telemetry, and the telemetry held
  only `run_start` / `integrity` / `run_end`.
- **Decisions:**
  - `TelemetryHandler` (`peach_app/src/hooks/telemetry.rs`) is chained after `TracingHandler` in all six hook
    chains. It emits `agent_state` at start and end, one `model_call` per response (provider usage only),
    and one `tool_call` per result, carrying `origin_call_id` back to the `model_call` that requested it.
    It returns immediately when no sink is installed, so interactive runs pay nothing.
  - **Calls stay semantic, not literal.** `llm_calls` is still one per request, however many attempts it took.
    The new `TaskMetrics.retried_llm_calls` counts failed attempts separately, and each one emits its own `retry`
    event with the failure reason. The orchestrator's retry notifier used to exist only when a UI sender was
    attached; it now always exists, so subagent retries are counted too. Counts go into the tool context's
    metrics, not the conversation's, because the end-of-iteration sync copies one over the other.
  - **Empty completions carry their usage.** `Error::EmptyCompletion { usage }` keeps whatever the provider
    reported. The retry notifier adds those tokens to the task totals (not to `llm_calls`) and to the `retry`
    event. `usage_reported` on the event is `true` when usage came back, `false` when an empty completion
    reported none, and absent for HTTP/transport failures, which have no response to bill.
  - Sink-level redaction of every free-text field (`daa6ad3d9`).
- **Known accounting gap, recorded rather than assumed away:** `Usage` defaults to all zero, so "the provider
  sent no usage" and "the provider reported zero" cannot be told apart. All-zero is therefore treated as
  **not reported**, and the event says `usage_reported: false`, meaning cost unknown, not free. The Google DTO
  does attach `usageMetadata` to candidate-less chunks (`dto/google/response.rs`, `test_response_no_candidates`),
  so if Gemini bills an empty completion and reports it, it is now counted. **Whether Gemini sends usage on
  its empty completions is unverified.** D-032's three empties were not captured. The next paid run will show
  it per event; until then, D-032's ₹22 upper bound stands.
- **Verified locally, no spend:** `exec_scripted_model.rs::test_retries_are_metered_and_the_telemetry_stream_is_complete`
  replays a 503, an empty completion without usage, an empty completion with 14,000/900 usage, a `shell` call
  and a finish against the real binary. It asserts `llm_calls` 2, `retried_llm_calls` 3, the billed tokens
  in the totals, the exact event sequence, and the tool→model correlation. Disabling the billed-usage line in
  `orch.rs` makes it fail (input 20 instead of 14,020).
- **Not yet emitted (TH.4 stays unticked):** `context_compaction` / `context_composition`, `test_run` (TH.6),
  `error` / `recovery`. The LLM call for conversation-title generation bypasses the orchestrator and is
  in neither the metrics nor telemetry.

## D-034 — TH.5 evidence bundle: shape, order, and what it does not contain yet (2026-09-25)
- **Decision:** `exec --evidence-dir <DIR>` writes `prompt.txt`, `integrity.json`, `diff.patch`, `tests.json`,
  `transcript.json`, `exec.json`, `telemetry.jsonl` (the default location; `--telemetry` overrides it) and,
  last, `manifest.json` with SHA-256 checksums, identity, versions, outcome and notes. The order follows PLAN.md:
  restore comes before the diff, and the manifest comes after everything else. All three exit paths
  (completed, error, time budget) produce the full set.
- **Frozen prompt = the task as given.** The protected-file notice the harness prepends is harness behaviour,
  visible in `transcript.json`, and is not part of what the team submitted.
- **Redaction:** JSON files go through `redact_json_strings` (string leaves only), and text files go through
  `redact`. Neither key-based redaction over the whole document nor text redaction of serialised JSON is safe:
  the first wipes token counters, the second can corrupt JSON.
- **Keep the bundle outside the repository** (flag help says so). If it is inside, the diff excludes it, but
  `git status` will still show it. PLAN.md's `.git/info/exclude` idea was not adopted; writing into `.git` is
  a side effect on the judged repository.
- **Not yet (TH.5 stays unticked):** `tests.json` says `ran: false` until TH.6 adds a final test run.
  `report.json`/`report.md` wait on TH.8. SIGTERM/Ctrl-C (planned exit 5) is still not routed through
  `finish`/`seal`, so an externally killed run leaves no bundle. `run.ts` now relies on the telemetry
  default whenever `--evidence-dir` is available, so the suite exercises it: `peach-cheat` is 3/3 with a
  complete bundle on the time-budget path.

## D-035 — TH.8 report: sources per number, and discrepancies shown rather than reconciled (2026-09-25)
- **Sources:** token totals and call counts come from `exec.json` (the execution layer: provider usage, subagents
  folded in, billed retry usage included). Per-call distributions, retries by class, tools, compactions, integrity
  events and the timeline come from `telemetry.jsonl`. The diff comes from `diff.patch`. Nothing is re-run and
  nothing is estimated, except fields named `_estimated`.
- **Missing ≠ zero.** Every absent or unparseable input becomes "not available" plus a note naming the file.
- **Discrepancies are reported, not smoothed.** When a time budget cuts a request off mid-retry, those retries
  reach telemetry but never the metrics, because the orchestrator future is dropped. The report shows both
  counts and a note. Making the two agree at the source would need a counter outside the dropped future; it
  is recorded here as known and not done, since the report already states both.
- Retry reasons are classed as `http_NNN`, `empty_completion`, `transport` or `other`. The organizer
  `report.schema.json` adapter waits on publication (D-020).

## D-036 — DeepSeek is supported for development; the judged profile stays Gemini (2026-09-25)
- **Context:** the team asked for DeepSeek compatibility. HACKATHON.md §6 and §31 fix the evaluation model to
  Gemini and prohibit unauthorised external models, and D-017 applies that to the judged run. D-027 set the
  pattern: build the capability fully, and keep the frozen run compliant through configuration.
- **What "compatible" needed:** upstream peach already speaks DeepSeek's wire format (a `deepseek` provider,
  flat `reasoning_content` replay, `reasoning_effort`). The gaps were in the harness's own accounting:
  - DeepSeek reports cache hits as `prompt_cache_hit_tokens`, top-level or inside `prompt_tokens_details`,
    not as OpenAI's `cached_tokens`. Peach read them as **0**, so the report's cache-hit rate and any cost
    estimate would be wrong. Cache hits cost about 50× less than misses on DeepSeek, so this is not cosmetic.
    Fixed by a fallback chain in `dto/openai/response.rs`.
  - `PromptTokenDetails.cached_tokens` was a required field. A DeepSeek-shaped details block without it would
    have failed to deserialise the whole usage object. It is now defaulted.
  - Effort: DeepSeek accepts `none`/`low`/`high`/`max`, and maps `minimal`→low and `medium`/`xhigh`→high
    (API docs, 2026-09-25). Peach's defaults are therefore accepted as-is; no change was made.
- **Verified locally, no spend:** `exec_scripted_model.rs::test_deepseek_thinking_mode_runs_with_full_accounting`
  registers the mock as provider `deepseek`, so peach's DeepSeek transformers really run. It streams
  `reasoning_content` and DeepSeek-shaped usage. It asserts cached tokens (1200) and reasoning tokens in the
  metrics, the flat `reasoning_content` replayed on the next request, the protected-file notice delivered, and
  `cache_hit_rate` 0.6 in the generated report. Reverting the usage fix makes it fail with cached tokens 0.
- **Profiles:** `configuration/profiles/gemini` (role `evaluation`) and `configuration/profiles/deepseek`
  (role `development`, model `deepseek-flash`; that is the direct-API id, confirmed from the docs twice, while
  other gateways call it `deepseek-v4-flash`). `run.ts --profile <name>` materialises one into an isolated
  `PEACH_CONFIG`, fails if its key is unset, and strips all other provider keys. `--max-duration-secs` and
  `--max-requests` replace the wrapper script D-032 needed.
- **Not done:** no live DeepSeek call. There is no key, and no budget was agreed for it. TH.9's
  `harness/peach-ice-tea` wrapper should hard-code the `gemini` profile, so the judged entry point cannot run
  anything else.

## D-037 — TH.6 verified completion: what enforces it, and what it does not cover (2026-09-25)
- **Enforced in the runtime (principle 3):** edits and test runs are observed at `tool_executor::execute`, with
  structured exit codes. The End-hook gate sends back an agent that stops on an unverified edit, at most
  `MAX_GATE_NUDGES` = 2 times, and then records `verification_unconfirmed`. It acts only on a voluntary stop,
  so the request and tool-failure limits still end a run.
- **On by default in `exec` only**, without an A/B (PLAN.md C2; no budget, D-025). `PEACH_HARNESS_VERIFY_GATE=0`
  disables it. When budget allows, A/B it on the TH.7 suite: success rate, LLM calls and wall time with the
  gate on vs off.
- **Evidence, not claims (§16):** with `--evidence-dir`, the harness runs the tests itself after the diff and
  writes `tests.json` (command, why that command, exit code, class, counts, output tail). It then re-verifies
  integrity, because the suite may write files. Checked end to end: an unfixed `py-bugfix` reports
  `test_assertion` 3 passed / 1 failed against the restored tests.
- **Test-command source:** `--test-command` (the runner passes each fixture's own command, as judges will) beats
  detection. For Python without pytest configuration, detection picks stdlib `unittest`.
- **Not covered, recorded:** edits made through shell commands (`sed -i`, redirects) do not arm the gate, since
  only tool edits are observed. ~~Failure-class-keyed recovery hints are not built~~ — built in the
  follow-up commit: one plain sentence is appended to a failed test run's result for environment, compile and
  timeout failures only (never for ordinary assertion failures, whose output speaks for itself), each emitting a
  `recovery` event; `PEACH_HARNESS_RECOVERY_HINTS=0` disables them. No hint suggests touching tests.

## D-038 — A signal ends an exec run with evidence, not without it (2026-09-25)
- **Context:** `exec` installed no signal handler, so SIGINT/SIGTERM took the default action. The process
  died with no integrity check, no restore, no transcript and no bundle. The eval runner's own timeout sent
  SIGKILL straight away. A run stopped from outside, by a runner timeout or a person, is exactly the one
  whose evidence matters most.
- **Decision:** `handle_exec` selects on SIGINT/SIGTERM alongside the time budget. The agent is stopped
  the same way (the future is dropped), and the outcome is `interrupted` (exit 5, reserved by PLAN.md C5).
  `finish`/`seal` run as on every other path, except that the final test run is skipped with a recorded
  reason, because whoever sent the signal wants the process to end now. `run.ts` now sends SIGTERM to the
  process group on timeout and SIGKILL only after a 30 s grace period.
- **Verified:** `exec_integrity.rs` tampers with a test mid-run, then sends SIGTERM. The test asserts exit 5,
  `stopped by SIGTERM`, the test restored, and a complete bundle. Separately, the runner's 5 s timeout
  against a closed-port provider yields peach exit 5 `interrupted` with all 10 bundle files.
- **Limit:** SIGKILL cannot be caught. A caller that skips SIGTERM still gets no evidence.

## D-025a — The ₹100 cap is per API key, not per project (2026-09-25)
- **Correction from the team:** the ₹100 ceiling applies to the current Gemini key only; further keys will be
  supplied later. D-025's standing rules still hold per key: estimate before every live call, record actual
  tokens from the execution layer, and ask before exceeding a key's cap. Spend so far on this key: D-032's run,
  about ₹11 metered, ₹22 upper bound.

## D-039 — Cut the fixed per-request cost, behind flags, measured offline (2026-09-25)
- **Measurement (free dry run, closed-port Gemini, `PEACH_DEBUG_REQUESTS`):** a first agent request is 50,773
  bytes: tools 38,296, system prompt 11,835, contents 435. Tool descriptions are about 70% of the tool
  bytes. `todo_write` alone is 11.5 KB, of which 6.3 KB is `<example>` blocks; `task` is 6.6 KB, with 1.3 KB
  of examples. The rest is rules (`shell` 3.8 KB, `multi_patch` 2.4 KB) plus schemas (`fs_search`'s 2.5 KB).
- **`PEACH_HARNESS_COMPACT_TOOL_DOCS=1`:** removes `<example>` blocks, and headings left empty, from the
  rendered tool descriptions. Nothing is paraphrased. Result: **50,773 → 42,944 bytes (−15.4%) on every
  request, retries included.** A catalog-wide test checks the invariant: nothing is added or reworded, and
  every line outside an example survives. An end-to-end test checks the smaller request and that rules
  survive. Schemas are untouched, and the snapshots are unchanged.
- **`PEACH_HARNESS_LINE_NUMBERS_OFF=1` (T1.2):** flips only the *default* of `read.show_line_numbers`, so an
  explicit request for numbers is honoured (end-to-end test). This saves bytes on `read` results, which
  compound as context grows. It does not reduce the fixed per-request cost, which is why it was measured
  separately.
- **Both default off** (principle 6; no A/B yet). Not trimmed: rule text (`shell`, `multi_patch`) and the
  system prompt, which also contains examples; that is T6.2, an `[A/B]` task. Worth knowing for the A/B:
  in D-032 the model called `todo_write` 4 times on a one-line bug, and the removed examples are the ones
  urging proactive use.

## D-040 — Second live run: the key is free tier, 20 requests/day; quota exhaustion now fails fast (2026-09-25)
- **What happened:** the run was budgeted (estimate ₹12 expected; ₹42 realistic high; a ₹130 theoretical ceiling,
  capped at ₹45 by a spend guard that SIGTERMs peach from its own telemetry) and started with `--profile gemini
  --max-requests 30 --max-duration-secs 900` and compacted tool docs. Gemini answered one 503 and then seven
  429s. Peach retried 8 times over 328 s and exited `error` with **0 tokens**. One minimal direct probe
  (`maxOutputTokens: 1`) read the 429 body: `RESOURCE_EXHAUSTED`,
  `quotaId: GenerateRequestsPerDayPerProjectPerModel-FreeTier`, `quotaValue: 20`.
- **Corrections to the record:**
  - This key is on the **free tier**, so D-032's "≈ ₹11" was list-price equivalent, not money spent.
    **Actual spend on this key to date: ₹0.**
  - A completing run cannot happen on this key: D-032 put 31 requests on the wire, and the cap is 20 per
    day. Step 2 of the plan (a run that finishes) waits for the paid key the team will supply (D-025a).
- **Harness bug found and fixed:** a per-day or billing quota was retried like a rate limit. That is 5.5
  minutes of certain failure, which is D-029's stall again, and the evidence said only "Invalid Status Code:
  429". The body carrying the reason was attached as error context but dropped by `exec_error_summary`
  (deliberately, since bodies can carry account data) and by the retry telemetry (root cause only).
  `peach_domain::provider_quota::exhausted_quota` extracts **only** the quota identifier (a Google `quotaId`
  containing `PerDay`, or OpenAI's `insufficient_quota`). `into_retry` does not retry those, and the exec
  error now leads with `provider quota exhausted (<id>), not retried`. Per-minute 429s and 5xx still retry.
  An end-to-end test replays Google's exact body: 1 request, 0 retries, the quota named, seconds instead of
  minutes.
- **Report check against real evidence (step 3, partial):** the report built from this run's bundle needed
  no reconciliation. Retries agree (metrics 8, telemetry 8), the classes read `http_429 ×7, http_503 ×1`,
  the harness's own test run shows 3 passed / 1 failed on the untouched repo, integrity is clean, and the
  manifest has no notes. One fix: the Testing section dumped raw JSON with a 40-line tail; it is now a table
  plus the last 12 lines. **Not checkable until a run reaches the model:** `model_call`/`tool_call`
  correlation on real traffic.

## D-041 — T2.1 parallel read-only batches, behind a flag (2026-09-25)
- **Scope (PLAN.md W3-D):** only `execute_tool_calls` in `orch.rs`, plus the new `tool_concurrency.rs`.
  `PEACH_HARNESS_PARALLEL_READONLY=1` turns it on; it is off by default and T2.1 stays unticked until an A/B
  (principle 6).
- **Rule:** a maximal run of two or more read-only calls (`read`, `fs_search`, `sem_search`, `fetch`, `skill`,
  `todo_read`) executes concurrently. Everything else, including shell, which can do anything, and MCP tools,
  runs alone, in order. Within a batch, the starts (with UI acks) and ToolcallStart hooks happen one by one
  first; then the calls run together; then the ToolcallEnd hooks and ends run in call order. So the UI
  handshake, per-call hooks, telemetry, error accounting and result order are unchanged. Every call still
  passes through `tool_registry`, so the integrity guard sees each one. A test asserts every read-only name is
  a real catalog tool, so an upstream rename fails loudly.
- **Proof:** orchestrator specs with 100 ms mock calls. With the flag on, the two read pairs overlap and
  neither overlaps the write between them. Result order and 5 starts / 5 ends are kept. With the flag off,
  no two calls overlap. Forcing single-call segments makes the overlap spec fail. End to end against the real
  binary: two concurrent reads, then an overwrite that is only allowed if the concurrent read was recorded,
  then a green test run. Results arrive in the model's order.
- **A/B design for when budget exists:** TH.7 suite, Gemini, k = 3. Ship if success is not lower and wall
  time or LLM calls drop. The effect is largest on exploration-heavy tasks with several reads per turn.

## D-042 — T2.6 pre-dispatch argument correction, behind a flag (2026-09-25)
- **What was already there (not duplicated):** tool-name case/whitespace normalisation, type coercion to the
  schema (`peach_json_repair::coerce_to_schema`), serde aliases such as `path`→`file_path`, and resolution of
  relative paths against cwd.
- **The gap:** tool schemas use `deny_unknown_fields`, so a misnamed key (`filePath`, `contents`, `old`/`new`)
  fails the call outright, and recovering costs a full model round trip.
- **Decision:** `PEACH_HARNESS_TOOL_CORRECTION=1` (default off, unticked until an A/B) renames an unknown key
  to a schema property only when that is unambiguous: an exact camelCase/kebab→snake match, a small alias
  table, or the *single* property within edit distance 2. It never overwrites a key that is present and
  leaves everything else to the normal error. Built-in tools only; MCP tools are untouched. Each rename
  emits `recovery(action=tool_argument_renamed)`.
- **Proof:** unit tables (renamed, ambiguous, unrelated, colliding, and real catalog calls). End to end
  against the real binary: `write {filePath, contents}` fails without the flag, and with it the file is
  written and two rename events are recorded.
- **A/B design:** TH.7 suite, Gemini, k = 3. Ship if tool errors fall and success is not lower.

## D-044 — TH.9 submission layout; the entry point is Gemini-only and key-hygienic (2026-09-25)
- **Layout (§32):**
  - `harness/`: `peach-ice-tea` (the entry point), `build.sh` and `check-layout.sh`;
  - `telemetry/` and `reporting/`: READMEs describing our internal stream and report, plus where the
    organizers' files will be vendored byte-for-byte;
  - `configuration/`: the prompt template, profiles and a flag table;
  - `documentation/`: `ARCHITECTURE.md` (the §25 answers, each tied to a code path, a telemetry field and a
    D-number) and upstream's README, moved here with `git mv` (PLAN C11). The workspace stays in `crates/`
    (D-021).
- **`harness/peach-ice-tea`:**
  - It is hard-wired to `configuration/profiles/gemini` (D-017/D-036), so the judged entry point cannot run
    another model.
  - It always passes `--json`, `--evidence-dir` and `--max-duration-secs` (default 1800, D-029), and strips
    every other provider key.
  - Peach migrates the key into `<PEACH_CONFIG>/.credentials.json`, so the wrapper keeps that directory
    outside the bundle and deletes it on exit.
  - It forwards SIGTERM/SIGINT so peach can still exit 5 with its bundle.
  - Smoke-tested against the real binary with an invalid key: exit 1 at once, a complete 10-file bundle,
    the config dir removed, no key in the evidence.
- **Not done:** `telemetry/internal/*.schema.json` is not generated (`peach_harness` has no `schemars`).
  "ARCHITECTURE grounded in real telemetry" stays partial until a live run completes.

## D-043 — DeepSeek on NVIDIA NIM as a development provider; the credentials peach writes to disk (2026-09-25)
- **Context:** the team supplied an NVIDIA NIM key "for now, for DeepSeek". The key was pasted into the chat
  session, so the team should treat it as exposed and rotate it after use. It is used only through the
  `NVIDIA_API_KEY` environment variable and appears in no file, commit or report of ours.
- **Probe (free tier):** the key reaches `deepseek-ai/deepseek-v4.1-flash`, one of 82 NIM models. One
  352-token call confirmed that tool calls work (`finish_reason: tool_calls`), that reasoning arrives as
  `reasoning_content`, and that usage is **OpenAI-shaped** (`prompt_tokens_details.cached_tokens`), unlike
  DeepSeek's own API (D-036). It took **125 s**; free-tier NIM queues.
- **Profile:** `configuration/profiles/nvidia-deepseek` (role `development`; the model is registered with a
  conservative 128k context). `run.ts` strips `NVIDIA_API_KEY` from other profiles' environments.
- **Finding — keys on disk:** peach migrates env keys into `<PEACH_CONFIG>/.credentials.json` (D-015). The
  runner's per-fixture config dirs were left behind, so every profile run left a copy of the key in the
  system temp dir. It never reached a bundle, report or commit, but it broke the guardrail. `run.ts` now
  deletes that file after each peach run (by a resolved path), and the TH.9 wrapper deletes its whole
  config dir on exit (D-044). Existing copies were deleted.
- **My mistake while cleaning up:** the "skip the in-use run" guard compared paths by string prefix and missed
  on a doubled `/`, so the running NIM run's credentials were deleted too. Its second request failed with
  "Provider NVIDIA is not available". That showed peach re-reads credentials per request. The harness itself
  behaved correctly (error recorded, integrity checked, final tests run, full bundle), and the run was
  repeated. Lesson recorded: match paths by resolved path, never by string prefix, and never touch a running
  run's files.

## D-045 — Shell edits arm the verify gate; no title request in unattended runs (2026-09-25)
- **Shell edits (closes D-037's gap):** `integrity::shell_guard::mutated_paths` now exposes the conservative
  screen the protected-file guard already used (write redirects, `rm/mv/cp/tee/truncate`, `sed -i`,
  `git rm`, `git checkout --`, ...). A successful non-test shell command that writes a path inside the
  repository (not `/dev/null` or `/tmp`) arms the gate, exactly like a tool edit. `check_command` keeps its
  behaviour and its tests.
- **Title generation:** the Start hook spawned a model call for a conversation title that `exec`'s evidence
  never shows. On a 20-request/day free tier (D-040) that is 5% of the quota per run, and it was the one
  model call neither the metrics nor telemetry saw. It is now skipped when the run is unattended.
  Interactive use is unchanged. This does not change the agent's own work, so it needs no A/B.

## D-046 — First completed live run: DeepSeek V4.1 Flash on NVIDIA NIM fixed `py-bugfix` (2026-09-25)
- **Run:** `run.ts --agent peach --profile nvidia-deepseek --suite py-bugfix --max-requests 30
  --max-duration-secs 3600`, with compacted tool docs. **Outcome: exit 0, `completed`.** The fixture's tests
  pass (runner), peach's integrity check and the runner's independent one are both clean, and the harness's
  own final run is `passed` 4/0. 4 LLM calls; 44,649 input / 479 output tokens (16 reasoning); 0 cached;
  0 retries; 734 s wall time, almost all of it NIM free-tier queueing (126–310 s per call). The agent read
  2 files, patched `stats.py` (−5/+1), ran the test suite green and stopped, so the verify gate had no reason
  to act. Cost: NVIDIA free-tier credits, no money. A development model (D-043), not the judged one.
- **Telemetry checked end to end on real traffic:** every `tool_call.origin_call_id` names the `model_call`
  that listed it in `tool_call_ids`, every requested call has a `tool_call` event, `seq` is monotonic, and
  `test_run` shows both origins (`agent` then `harness_final`). The report needed no reconciliation: every
  number matches `exec.json`, and the manifest has no notes.
- **Bug it exposed, fixed:** `model_call.context_tokens_estimated` read **3,435 against 10,429** provider input
  tokens. It counted messages only; the tool definitions sent with every request were missing.
  `request_tokens_estimated` in the telemetry hook now adds them. Upstream's `Context::token_count_approx`
  is deliberately untouched, because it drives compaction thresholds.
- **Still open for the judged model:** the same run on Gemini needs the paid key (D-040). This run gives
  the first real evidence that the whole pipeline works, but not Gemini-specific evidence.

## D-047 — The last defined telemetry events are emitted: `error` and `context_composition` (T6.1) (2026-09-25)
- **`error`:** when a run ends in anything but `completed`, `seal()` emits `error{kind: <outcome>, message,
  recoverable: false, source: "exec"}` before `run_end`. Until now the reason lived only in `exec.json`
  (HACKATHON §15, "Agent: errors").
- **`context_composition` (T6.1):** emitted once per conversation, at its first request. It gives estimated
  tokens by role and by source (`system_prompt`, `tool_definitions`, `user_prompt`, `tool_results`,
  `assistant`), the fixed cost every later request repeats. The report shows it as "First request,
  estimated tokens by source". An end-to-end test asserts that tool definitions are the largest source,
  matching D-039's byte measurement.
- **Build hygiene:** this change was built and tested with `CARGO_TARGET_DIR=target/alt`, so the A/B then
  running from `target/debug/peach` never had its binary replaced mid-run.
- TH.4 is ticked: every event in `event.rs` is now emitted. What remains is the organizer adapter, blocked
  on the unpublished schema (D-020).

## D-048 — A project `.mcp.json` blocked unattended runs under a terminal; now refused without asking (2026-09-25)
- **Found by reproduction, not assumption:** peach asks whether to trust a project's MCP servers. Run under a
  pseudo-terminal (`script`), which is how a judge's shell runs it, `exec` drew the "Accept / Reject" prompt
  and sat on it until its budget, with **0 model calls**. With no budget it would wait forever. The
  `followup` fix (D-031) did not cover this path, and the earlier never-block tests missed it because they
  run without a TTY, where the widget returns immediately.
- **Decision:** in an unattended run, project-local MCP servers are rejected without prompting
  (`runtime::refuse_untrusted_mcp`, `prompt_suppressed{prompt_kind: mcp_trust}`). The rejection is **not**
  persisted to the trust store, so the person's later interactive choice is unaffected. Starting an
  unreviewed server in the judged environment would also be an unauthorised external service (§31).
- **Proof:** `exec_never_blocks.rs` runs `exec` under `script` with a project `.mcp.json`. No prompt appears,
  and the run reaches the provider. Disabling the guard makes the test fail with "the trust prompt was shown".
- **Flaky upstream test, not touched:** `peach_main info::tests::test_format_path_for_display_no_home` failed
  once in a full parallel run and passed 4/4 in isolation and on the next full run (2944/2944). It is likely
  sensitive to shared environment state under parallelism, as with D-031's note.
