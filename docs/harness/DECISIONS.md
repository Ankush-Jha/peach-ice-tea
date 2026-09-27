# Decision log

Append-only. Format: ID, date, decision, context, alternatives, consequences. Claude Code:
when the spec is silent or ambiguous, **make the call, record it here, and continue** —
don't stop to ask unless the choice is destructive, irreversible, or changes scope.

## D-001 — Fork an open-source agent rather than wrap it (2026-09-20)
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

## D-005 — Rebuild Hosted Services capabilities in the open, selectively (2026-09-20)
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
  release. The README states plainly that Peach Ice Tea is built on a fork of an open-source (Apache-2.0) coding agent — which also answers ALIGNMENT §4
  Q1 honestly rather than obscuring it. Rebranding the binary and banner is a later, separate decision if wanted.

## D-024 — A a fork of an open-source (Apache-2.0) coding agent is eligible; provenance stays explicit (2026-09-23)
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

## D-049 — Any model may be used; models are chosen per role, from evidence (2026-09-26)
- **Context:** the team lifted the single-model constraint ("we can use any model now"). D-017, D-027, D-036 and
  D-044 pinned the judged run to Gemini because HACKATHON.md §6/§31 fixed the model; that premise no longer holds.
  Money is unchanged: the Gemini key is free tier at 20 requests/day per model (D-040), which cannot complete one
  run (D-032 needed 31), and the OpenRouter key has **$0 credit**, so only `:free` models are callable there.
- **Probe (2026-09-26, free, one tool-call request each):**

  | Model (OpenRouter) | Result | Latency |
  |---|---|---|
  | `nvidia/nemotron-3-ultra-550b-a55b:free` (1M ctx) | correct `read` call | 1.9 s |
  | `cohere/north-mini-code:free` (256k ctx) | correct `read` call | 1.0 s |
  | `qwen/qwen3.8-27b:free`, `poolside/laguna-s-2.1:free` | 429 upstream rate limit | – |
  | `thinkingmachines/inkling:free` | 403: allow-listed apps only; not spoofed | – |

  For comparison, NIM DeepSeek took 126–310 s per call (D-046).
- **Decision:**
  1. **Roles, not one model.** Each role gets the model whose strength fits it: the main `peach` agent gets the
     strongest reasoning/tool-use model; `sage` (read-only exploration) and compaction summaries get a fast,
     cheap, long-context model; the relevance scorer (T3.9) gets the cheapest model that answers reliably.
     Peach already carries provider+model per agent and a compaction model, so this is mostly configuration;
     the runtime part is MM.3.
  2. **The default model is picked by a bake-off on the TH.7 suite (MM.2), not by reputation.** Until then the
     wrapper takes `--profile`, and the Gemini profile stays available unchanged.
  3. **Principle 6 becomes satisfiable at no cost:** the second model family for every pending `[A/B]` is a
     free OpenRouter model. Flags whose A/B holds on both families flip to default on.
- **Budget:** superseded by D-050 the same day.

## D-050 — Token cost is not the constraint; pick models on merit (2026-09-26)
- **Team direction:** "We are not bearing the cost of the token … just open the field." D-025/D-025a's ₹100 cap and
  the "free models only" line in D-049 are withdrawn. Nothing about the model or provider is assumed to be given in
  advance; the harness must be good on whatever model it is pointed at, and we pick the best per role.
- **Checked live — and corrected the same hour:** 16-token requests to `anthropic/claude-opus-5.5`, `openai/gpt-6-sol`
  and `google/gemini-3.8-flash` succeeded, which I first read as "paid models work". **They do not, for agent turns.**
  The account holds **$0 credit**; OpenRouter admits a request only if its worst case (`max_tokens`) is affordable.
  Peach asks for 20,480 output tokens, so every real turn gets `402 … can only afford 1354` (bake-off round 1: 6 of
  9 models failed every fixture with 0 LLM calls). The key's $50 figure is a daily *limit*, not a balance.
  **Paid models need credit added to the account.** Until then only `:free` models (50 requests/day) are callable.
- **Harness fix from this:** a 402 with `limit_source: openrouter_credits` is recognised as an exhausted quota, so
  `exec` says `provider quota exhausted (openrouter_credits), not retried` instead of `Invalid Status Code: 402`.
- **Consequences:** `[A/B]` tasks get real A/Bs at k = 3 on two families (principle 6 is now met normally, not
  waived); the bake-off (MM.2) includes frontier models; the per-role routing (MM.3) is judged on success first,
  then cost.

## D-051 — Bake-off round 1: only account limits failed; Nemotron Ultra is the default for now (2026-09-26)
- **Round 1** (`benchmarks/reports/models/20260926-round1.md`): 9 models × 6 fixtures × 1 seed through the real
  binary. Six models (Fable 5.1, Opus 5.5, GPT-6 Sol, Gemini 3.8 Flash, Grok 4.7, Qwen 3.8 Max) got 402 on every
  first request: no credit (D-050). Every run that reached a model passed (Nemotron 5/5 + smoke, GLM 4/4, DeepSeek
  Pro 1/1). Spend: **$0.065** total.
- **Decision:** the `openrouter` profile and the `harness/peach-ice-tea` default use
  `nvidia/nemotron-3-ultra-550b-a55b:free`, the one model with a full passing record that runs without credit.
  Cost: 40 calls / 684k input tokens for 5 fixtures, about 2× GLM 5.3's calls, so it is the default because it
  runs, not because it is efficient. When credit exists, round 2 reruns all nine and the shortlist gets k = 3.
- **Limit found:** the free tier is 50 requests/day across all free models; one six-fixture suite needs ~50.

## D-052 — MM.3 per-role routing: what routes, what cannot, and a failure the tests found (2026-09-26)
- **Organizer confirmation (via the team):** any model is allowed in the judged run. D-017's compliance concern is closed.
- **What actually calls a model in `exec`:** the main loop (`orch.rs`) and subagents (`agent_executor.rs`). Title
  generation is skipped when unattended (D-045). **Compaction calls no model:** `Compactor::compact` renders a
  template summary, and `compact.model` is carried in config but never read by a model call. So "a cheap compaction
  model" would be dead configuration; it was removed from `openrouter-routed`. An LLM-written summary is new
  behaviour (T3.10 territory, `[A/B]`), not routing. The relevance-scorer role likewise waits on T3.9, which has no
  `LlmScorer` yet. **Routing today therefore means `roles.<subagent>`**, e.g. `sage` on a fast long-context model.
- **Proof, end to end with two scripted providers** (`exec_scripted_model.rs`): with `roles.sage` set, the sage
  request carries `fast-model` and every main-loop request carries `scripted-model`. Ignoring `roles` in the resolver
  makes this test fail.
- **Failure found by the second test:** when the role model's provider failed, the parent received upstream's
  generic tool-error frame: "reflect on what was wrong with the tool call … make the correct tool call", with 2
  attempts left. That tells the strong model to retry a delegation that cannot succeed, which spends turns and can
  reach the tool-failure limit, and it never said which model failed. `AgentExecutor` now prefixes the cause with
  "The 'sage' subagent's model (<provider>/<model>) failed … a provider failure, not a mistake in your tool call …
  Continue the work yourself", and emits `recovery{action: subagent_model_failed}`. The run completes (exit 0).
  Automatic re-run on the session model was considered and not built: `PeachApp::chat` resolves the model from the
  agent id, and a per-request override would touch upstream's hot path (principle 8). The parent is the strongest
  model and has the same read tools, so handing the work back is the cheaper fail-open.
- **Not yet shown:** whether routing helps. That is the `[A/B]` (routed vs single-model), which needs credit (D-050).

## D-053 — A `confirm` permission stalled unattended runs under a terminal; now refused, and TH.1 is complete (2026-09-26)
- **Where I stopped on the brief:** steps 1–3 (bake-off round 2, D-046 reproduction on the new default, k = 3 A/Bs)
  are blocked. The OpenRouter account still has $0 credit (checked 15:34 UTC), and the free allowance is spent
  (52/50; it resets daily). A background watcher polls for credit. Step 4 needs no model calls, so it continued.
- **Found by reproduction:** with `restricted = true` and a `confirm` rule in `permissions.yaml`, `policy.rs` calls
  `select_one_enum` mid-run. Under a pseudo-terminal (`script`) an `exec` run sat on it until its 20 s budget
  (`time_budget`, task not done); without a budget it waits forever. The wrapper's fresh config dir hides this,
  but plain `peach exec` on a machine with such a file does not.
- **Decision (D-022 applied):** in an unattended run, a `confirm` is **refused** without asking
  (`runtime::refuse_permission_prompt`, `prompt_suppressed{prompt_kind: permission, default_action: refused}`).
  It is refused rather than allowed because the rule's author wanted a person to decide, and none is present.
  The model was also told "User has denied the permission"; there is no user, and "denied" invites asking again.
  Unattended runs now get a plain statement of what happened and what to do (principle 4). Interactive
  behaviour is unchanged.
- **Proof:** `test_a_confirm_permission_is_refused_not_asked_in_an_unattended_run` (under `script`): `completed`
  in ~2.5 s, event recorded, and the model receives the new text. Before the fix the same test ended at
  `time_budget` after 22 s.
- **TH.1 audit (a test per path, R-HACK-1):** continue-anyway (new TTY test: `request_limit`, no prompt),
  `followup` (D-031), permission (this), MCP trust (D-048), pickers/login (missing config and unknown provider
  fail fast), provider down (fails fast within budget). The update prompt is unreachable by construction:
  `init_state_exec` never calls `on_update`. **TH.1 ticked.**
- **Flaky upstream test, not touched:** `peach_app fmt::fmt_output::tests::test_fs_create_overwrite` failed once
  in a full parallel run, passed 3/3 alone, and the next full run was 2952/2952.

## D-054 — Test sections of mixed files are watched and flagged; TH.2 is complete (2026-09-26)
- **Gap (R-HACK-2):** `package.json`, `pyproject.toml`, `Cargo.toml` and `setup.cfg` cannot be protected wholesale,
  because adding a dependency is legitimate. R-HACK-2 requires their test sections to be detected after the run and
  flagged. Nothing did this: a model could replace `"test": "node --test"` with `"test": "true"` and the only
  signal would be the runner's git diff.
- **Built (`integrity/mixed.rs`):** before the run, each such file's *test sections* are rendered canonically:
  - `package.json`: scripts whose name contains `test` (`pretest`, `test:unit`), plus the jest, mocha, ava,
    vitest, c8 and nyc keys;
  - `pyproject.toml`: `tool.pytest`, `tool.tox`;
  - `Cargo.toml`: `[[test]]`, `profile.test`;
  - `setup.cfg`: `[tool:pytest]` / `[pytest]`.

  TOML goes through a parse-and-reserialise round trip, so formatting and comments do not count as a change.
  Excluded trees such as `node_modules` are skipped. After the run a difference becomes a violation of the new kind
  `test_config_changed`. It is **flagged, never restored**, because the rest of the file may be a legitimate edit.
  It flows through the existing report, `exec.json` and `integrity` telemetry (`violation`).
- **Told up front (principle 4):** when such sections exist, the model's protected-files notice adds "the test
  sections of these files must not change: …, other edits are fine".
- **Proof:** unit tests (non-test edits ignored; script, runner-config, pytest `addopts` and `[[test]]` changes caught;
  reformatting ignored). End to end, a scripted model rewrites `package.json`'s `test` to `true`: the notice is in
  the first request, and `integrity.violations == [{package.json, test_config_changed, restored: false}]`.
- **Known limit, recorded:** the harness's own final test run (TH.6) calls the detected command (e.g. `npm test`),
  which reads the *current* script. After such a cheat it could report a pass; the evidence now carries the
  violation beside it, so a judge sees both.
- **TH.2 ticked:** globs, tool refusal, shell detection, manifest diff and restore (unit), dispatch refusal
  (scripted model), restore on every exit path including signals (D-030, D-038), and mixed files (this).
- **Upstream flake, root cause found:** `peach_app` `test_fs_create_overwrite` (seen again). `todo_fmt.rs` tests
  toggle `console`'s process-global colour flag. When one runs between this test's `actual` and `expected` renders,
  one string has ANSI codes and the other does not. A race between upstream tests, unrelated to our code; recorded,
  not changed (CLAUDE.md guardrail).

## D-055 — TH.3 and T0.8 closed; R-TOOL-1's ordering rule stays deferred, now with the concrete reason (2026-09-26)
- **TH.3 (R-HACK-6), each item checked against the code:**
  - `thoughtsTokenCount` folded into output and kept as reasoning (D-026, confirmed live);
  - `cachedContentTokenCount` read (`dto/google/response.rs`);
  - every catalog tool schema, after conversion, passes a Gemini-allowed-keys/format walk (`schema_compat.rs`, plus
    an MCP-shaped fixture and snapshots);
  - thinking-level mapping wired (`20500d10f`, found in D-032's dry run);
  - `gemini-3.8-flash` registered in `configuration/profiles/gemini`.

  "Registry becomes Gemini-only" is superseded by D-049/D-052: any model is allowed in the judged run.
- **`max_tokens = 20480` vs Gemini's 65,536 output limit (raised in D-026):** not changed, because there is no
  evidence it binds. D-032's only live Gemini run used 8,296 thinking tokens across 13 calls (well under 1k per
  call). Its 3 empty completions cannot have been budget exhaustion either: a Gemini `MAX_TOKENS` finish maps to
  `FinishReason::Length`, and peach raises `EmptyCompletion` only when there is *no* finish reason. Revisit if a
  `Length` finish appears in a real bundle's telemetry.
- **T0.8 (R-TOOL-1):** the flat-schema rule is enforced by a catalog-wide test. The "`required` before
  `properties`" rule was deferred in PLAN C14 but never recorded here, so it is recorded now, with the reason. The
  workspace `serde_json` lacks `preserve_order`, so every JSON object is serialised with sorted keys, and
  `properties` < `required` alphabetically. Reordering means turning `preserve_order` on for the whole workspace,
  which changes key order in every serialised payload for every provider (and churns snapshots), and the ordering
  evidence is GPT-specific. With GPT models now in scope (D-049), it becomes an `[A/B]` candidate on a GPT model,
  not a default. The documenting test `test_t0_8_required_currently_orders_after_properties` will notice if this
  changes. **T0.8 ticked** (enforced part done; ordering deferred by decision).

## D-056 — A SIGKILLed run now leaves a bundle that says so and can be restored from; TH.5 closed (2026-09-26)
- **Gap:** TH.5's one recorded hole was SIGKILL (D-038). Nothing can run after it, and everything except
  `prompt.txt` and streamed telemetry was written at the end, so a killed run left no manifest. A missing
  manifest looks the same as "never ran". The integrity snapshot survived in the temp dir, but nothing said
  where it was, so a tampered test could not be restored from the bundle.
- **Fix:** at start, right after the integrity manifest is captured, the bundle gets `integrity.baseline.json`
  (each protected file's pre-run hash and size, the snapshot location, and the mixed-file test sections) and a
  **provisional `manifest.json` with outcome `incomplete`**. Its note says the process was killed and points at
  the baseline. Every normal exit, signals included, overwrites it via `seal()`. The note is not carried into
  the final manifest. Evidence layout version → 0.2.0.
- **Proof:** `exec_integrity.rs::test_a_sigkilled_run_leaves_a_bundle_that_says_so_and_can_be_restored` tampers
  with a test mid-run, then sends SIGKILL. The bundle's manifest reads `incomplete`, and the baseline lists the
  test's original size and a snapshot copy with the original content. It failed before the change (no
  `manifest.json`). The completed-bundle test's file list gains `integrity.baseline.json`: an intended layout
  change, not a weakened assertion.
- **TH.5 ticked.** The residual limit is inherent: after SIGKILL there is no transcript, report or automatic restore.

## D-057 — The local suite gains integration tests; TH.7 closed (2026-09-26)
- **Gap:** R-HACK-8 requires fixtures with unit *and* integration tests, since the judged repository has both
  (HACKATHON §7). `run.ts` has supported `integration_command` since TH.7, but no fixture set one: every run
  printed `integration=n/a`, so an agent that passed unit tests but broke the program end to end scored as a success.
- **Added:** integration suites in the three harder fixtures, each exercising the program across module
  boundaries and failing on the unfixed code:
  - `py-config-cli`: runs `python -m synctool` as a subprocess against a real config file (comments, `dry-run`
    key, CLI override);
  - `py-ledger`: a discounted invoice with a half-cent line price, rendered and split three ways to the cent;
  - `js-duration`: a new `src/jobs.js` loads a jobs JSON file and resolves fractional timeouts. It names the job
    on error.

  The integration files are declared in `test_files`, so both integrity checks cover them. For Node, a directory
  argument to `node --test` fails on v25 (it is resolved as a module), so the command uses a glob.
- **Checked:** `--agent reference` gives 3/3 with integration pass, and each suite fails before the fix.
  `--agent peach-cheat` (real binary, tests tampered mid-run) gives 3/3 restored, integrity clean. The original
  three fixtures stay unit-only: they are one-function bugs, and their value is as fast smoke tests.
- **TH.7 ticked.**

## D-058 — Two of six fixtures had no JavaScript in git (2026-09-26)
- **What broke:** the root `.gitignore` ignores `*.js` (build output), and that also matched the fixture
  repositories' source and tests. `node-feature` (since TH.7) and `js-duration` (since `cfabb11b9`) were committed
  without any `.js` file. Every run here passed because the files existed in this working tree. Any fresh clone
  (a teammate, CI, a reviewer under HACKATHON §33) got two fixtures with nothing to test.
- **Found by:** `git status` after adding `js-duration`'s integration test showed `meta.json` staged but not the new
  `.js` files. `git check-ignore -v` pointed at `.gitignore:43`, and `git ls-files` showed the same gap in `node-feature`.
- **Fix:** one negation, `!benchmarks/hackathon/fixtures/*/repo/**/*.js`, scoped to fixture repositories.
  **Verified from a fresh clone** of `f4db04837`: `run.ts --agent reference --suite all` gives 6/6, including
  integration. An ignored-files sweep of the harness directories found only `.DS_Store`. Per-run artifacts under
  `benchmarks/reports/hackathon/` are ignored by design (TH.7); aggregate A/B and bake-off reports are committed.
- **Process slip, recorded:** `f4db04837` was committed without running clippy first, against the standing rule.
  It changed no Rust. Clippy was run right after (clean) and is included with this entry's commit.

## D-059 — MCP output was never clipped: the shaper existed but nothing called it; T1.1 and T3.7 closed (2026-09-26)
- **What broke:** `712689fc9` (T1.1) built `truncation::shape_mcp_output` with 5 tests, left it `#[allow(dead_code)]`
  because `tool_registry.rs` belonged to another piece, and listed the call site as carried forward. It was never
  added. MCP tool results reached the model **unclipped and unannounced**: one large MCP result could fill the
  context with no notice and no recovery path, the opposite of R-OUT-4.
- **Found by:** reading T1.1's commit for its "unfinished" list; `grep shape_mcp_output` showed tests only.
- **Fix:** called in the MCP branch of `ToolRegistry::call_inner`, right after the executor returns. It uses the
  shell caps, with head/tail clipping, the shared recovery sentence and the full text saved to a temp file for
  `read`. The `dead_code` and `unused_imports` allowances are removed, so the compiler now flags it if it becomes
  unused again. In unattended runs project MCP servers are refused (D-048), so this matters for interactive use
  and globally trusted servers.
- **T1.1 ticked:** read, shell, search and fetch have snapshot tests containing the notice, and MCP has unit
  assertions. The behavioural "truncation awareness" test is R-EVAL-4's (T0.7), tracked there.
- **T3.7 ticked** (no code change): `peach_harness::redact`, 36 tests, covers R-SAFE-3's keys (`access_token` via
  `token`; hyphen/camel variants normalised) plus credential formats, and is applied at every exit that exists
  today: evidence files, telemetry free text and scorer previews. T6.3's external hooks must call it when built.
- **Still carried from `712689fc9`:** `first_error_recovered` is summed and diffed but derived nowhere (T1.3).

## D-060 — `offload_read` could never be non-zero; `first_error_recovered` was never set; T1.3 closed (2026-09-26)
- **What broke:**
  - `TaskMetrics::record_read` counts `offload_read` only for paths in `dump_files`, and
    `record_dump_file` had **no caller anywhere**. `TempContentFiles` even carried a comment saying its paths
    were exposed "to record them as recovery dump files", but nothing recorded them. So the counter that tells
    us whether withholding output cost the agent a turn (principle 1; the ship bar for T1.2 and T1.5) read 0 in
    every run, whatever happened.
  - `first_error_recovered` was summed across subagents and diffed, but derived nowhere (carried from `712689fc9`).
- **Found by:** following T1.1's shaper wiring (D-059) to its R-OUT-3 half: reading an MCP dump would not count,
  and `grep record_dump_file` showed no callers for shell and fetch either.
- **Fix:**
  - The tool executor registers every dump file `dump_operation` creates (shell stdout/stderr, fetch).
  - `shape_mcp_output` now returns its dump paths, and the MCP branch registers them.
  - `ExecReport::new` derives `first_error_recovered = completed && any tool error`, in the same place `exit_code`
    is derived from the outcome, so the two cannot disagree.
- **Proof:**
  - End to end, a scripted model runs `seq 1 500` (truncated). A new `Turn::FromRequest` then builds the next call
    from the request body, reading the path in the "Full output: read …" notice, as a real model would.
    `metrics.recovery.offload_read == 1`, and the read returns the withheld middle.
  - Removing the registration makes that test fail.
  - Unit tests cover MCP returning its handle (with the file's content) and `first_error_recovered` for
    completed/failed/clean runs.
- **Consequence for the A/B record:** every earlier report's `offload_read` (including
  `2026-09-25-compact-tool-docs-nim`) was structurally 0 and says nothing about recovery. The pending A/Bs will be
  the first real measurement.
- **T1.3 ticked.** Handles: shell, fetch and MCP dump to a file; read's truncation points at the file itself with a
  line range; search's points at a re-run with an offset.

## D-061 — The heuristic scorer ignored whether a result was still being talked about; T3.8 closed (2026-09-26)
- **Gap:** R-CTX-4 specifies `HeuristicScorer` as: keep errors, **results whose paths or identifiers appear in later
  assistant or user text**, and non-idempotent commands; drop successful idempotent reads that were never
  referenced. The implementation had recency, status, size and re-runnability, but no reference signal.
  `ToolCallSummary` had no field for it. So a large `read` of the file the agent was actively fixing scored like
  an unused read and would have been truncated. That is the compaction failure principle 1 warns about: it costs
  a re-read.
- **Fix:**
  - `ToolCallSummary.referenced_later` (serde default `false`, builder setter);
  - `plan::is_referenced_later(input_preview, later_text)`, a pure function. It takes path-like tokens from the
    call's input (containing `/`, or a name with an extension, at least 4 characters) and matches each in full or
    by file name, since models write `stats.py`, not `/repo/src/stats.py`. Plain words (`echo hi`) never match;
  - in the heuristic, a referenced call gets a 0.8 floor on both scores, above the 0.5 threshold, so its decision
    is `Keep`.
- **Tests:** matching by full path and by file name; no match for plain words or 3-character names; a referenced
  20 kB read is `Keep` while the identical unreferenced read is not. The existing fake-scorer tests (failing scorer
  keeps everything, a missing answer keeps that call, pinning, redaction before scoring, stats) already covered the
  rest of T3.8.
- **Not wired yet (by design):** the compaction pipeline that builds summaries and calls a scorer is T3.4. The
  builder must compute `referenced_later` from the messages after each call. **T3.8 ticked.**

## D-062 — T3.4: compaction runs through a staged pipeline; behaviour identical, proven by a golden test (2026-09-26)
- **Built (R-CTX-2):** `peach_app/src/compaction_pipeline/`. `Pipeline` runs an ordered list of `Stage`s, stopping
  once the context is at or below the target, except that the final stage always runs if reached. It reports
  `stage_reached`. `Stage` is an enum rather than trait objects (AGENTS.md: no `Box<dyn>`). Its one variant today
  is `Summarize`, the existing `Compactor`. S0 supersede, S1 offload and S2 score are `[A/B]` tasks (T3.5, T3.6,
  T3.9) and join as variants ahead of it. `hooks/compaction.rs` calls the pipeline instead of `Compactor`
  directly. The manual `/compact` path in `app.rs` (full compaction, `max = true`) is untouched.
- **Proof of "behaviour identical":** a golden test runs the pipeline and a direct `Compactor::compact(ctx, false)`
  over four conversation shapes (empty, one message, 12 read turns, the same plus a new request) and requires
  equal output. A guard requires at least two fixtures to actually change, so two no-ops cannot pass as equal.
  The end-to-end compaction telemetry test still passes.
- **Flaky upstream test:** `test_fs_create_overwrite` has now failed in 3 full runs (D-054 root cause, a
  global colour-flag race). Still not modified, per the guardrail. If it keeps recurring, the fix belongs upstream
  in `todo_fmt`'s colour guard.

## D-063 — T3.10: a deterministic handoff note tops the compaction summary, behind a flag (2026-09-26)
- **Why (R-CTX-6):** peach's S3 summary paraphrases the evicted turns. Four kinds of fact must survive exactly or the
  agent re-does or undoes work: what is left to do, what the user insisted on, what has already been changed, and
  what last failed.
- **Built:** `compaction_pipeline::handoff::handoff_note(context, todos, changed_files)`, pure and deterministic,
  with no model. It has four sections, each omitted when empty:
  - the todo list with statuses;
  - user messages containing must, never, always, don't, do not, only, should not or required, kept verbatim
    (≤ 600 characters each, the newest 8), matched as whole words (so "commonly" ≠ "only"), with earlier notes
    skipped so they do not nest;
  - files changed so far (`file_operations` that are not reads);
  - the last shell command with a non-zero `exit_code`, with an output excerpt.

  `Pipeline::handoff_note` puts it at the top of the one message S3 adds, found as the message that was not in the
  context before the stage. "Open questions" from R-CTX-6 is not built: it cannot be derived deterministically,
  and the spec reserves a model only for polish.
- **Flag:** `PEACH_HARNESS_HANDOFF_NOTE=1`, **default off**. It changes what the model sees after every compaction,
  so it waits for an A/B like the other flags (principle 6). The A/B only means something on tasks long enough to
  compact (T3.12's ≥ 60-turn suite), or on the six fixtures with a lowered message threshold.
- **Proof:**
  - Unit: the exact note text for a mixed context; no note when there is nothing to hand off; whole-word markers.
  - Pipeline: with a note, exactly one message (the summary) differs from the plain run, and it is the note plus
    the plain summary.
  - End to end: a compacting run whose prompt says "you must not rename add" has "HANDOFF NOTE" and that sentence
    in its later requests with the flag, and no note without it.

## D-064 — T3.1: an append-only event log and artifact store; replay reproduces the pre-compaction context (2026-09-26)
- **Why (R-CTX-1):** `conversations.context` is the only copy of history, and compaction overwrites it. Reversible
  compaction (T3.5, T3.6) and `recall` (T3.3) need the originals to still exist somewhere.
- **Domain (`peach_domain::thread_event`, pure):**
  - `ThreadEvent::Message { entry }` holds any context entry exactly: user, assistant with reasoning and tool calls,
    or tool result.
  - `ThreadEvent::Compaction { messages_before, view }` records the new working view. It deletes nothing.
  - `replay_history` returns every message ever added; `replay_view` gives the working view with compactions applied.
  - `events_between(before, after)` turns two snapshots into events (appends become messages; a rewrite becomes one
    compaction). This lets T3.2's writer work from the snapshots the orchestrator already has.
  - `ThreadEventRepository` trait.
- **Storage (`peach_repo::ThreadEventRepositoryImpl`):**
  - Migration `2026-09-26-000000_create_thread_events_and_artifacts` creates
    `thread_events(conversation_id, seq, turn_id, kind, payload_json, artifact_hash, created_at)` (PK conversation_id
    + seq) and `artifacts(hash PK, bytes, mime, size, created_at)`. `artifact_hash` is an addition to the spec's
    columns, so GC can run by index.
  - Appends run in one transaction with gap-free sequence numbers.
  - Payloads of 16 KiB or more go to the artifact store, keyed by SHA-256 and deduplicated.
  - **Cap 512 MiB.** An artifact that would exceed it stays inline in its row, so only deduplication is lost, never
    data.
  - `gc_artifacts` deletes artifacts that no event of a live conversation (one whose row exists) references.
  - Event payloads are domain JSON, not repository-mirror records like `conversation_record.rs`. Replay has to be
    exact, and a mirror would add a translation that could drift. The cost is that a breaking change to
    `MessageEntry`'s serde shape breaks old logs; `list_events` then fails with the seq and the decode error, and
    never returns a guessed event.
- **Acceptance, tested:**
  - Replaying a stored log (messages, then compaction) reproduces the pre-compaction messages byte for byte
    (serialised JSON equal), and the view equals the compacted context.
  - Two identical large payloads create one artifact.
  - Over the cap, the payload is stored inline and read back intact.
  - GC deletes the orphan's artifact and keeps the live one's.
  - A conversation with no events (every conversation before this migration) reads as empty, not as an error.
  - The migration is additive (`CREATE TABLE IF NOT EXISTS`), so existing databases upgrade in place.
- **Not yet (T3.2):** nothing writes events during a run. The orchestrator append path, and making
  `conversations.context` a projection, is the next task.

## D-065 — T3.2: every save appends to the event log; two ways a snapshot diff lied, found on a real run (2026-09-26)
- **Write path:** `PeachRepo::upsert_conversation` saves the working view first, which stays authoritative. It then
  hands the conversation to `EventLogWriter`, which appends the events between the log's view and the new context.
  - A logging error is a warning and never fails the save (principle 5).
  - The writer diffs against the **log's own replayed view**, cached per conversation and rebuilt from the log on a
    miss, never against the stored `context` column: that goes through repository mirror records, and any
    difference in the round trip would read as a rewrite.
  - A resumed conversation continues its log. A conversation from before the migration is seeded on its first save.
  - `conversations.context` is now a projection: `replay_view(log)` equals it (writer test).
- **Found by the end-to-end test, not by the unit tests:** a real `exec` run that compacts, inspected with `sqlite3`.
  - **Lie 1: metadata looked like compaction.** peach edits messages it has already saved. `SetModel` stamps the
    model on text messages that lack one, and usage is attached after a response. The exact-prefix check read each
    of these as a rewrite, so the first save after the prompt logged a "compaction" of 3 messages into 3.
    **Fix:** messages match by content, ignoring `model` and `usage`. A metadata-only change becomes a small
    `Revise { index, entry }` event, applied by `replay_view` (so the view stays byte-exact) and ignored by
    `replay_history`.
  - **Lie 2: messages hidden inside compaction views.** peach compacts mid-turn (`on_response`) and saves at the
    end of the turn, so one snapshot holds a compaction *and* that turn's new call and result. They were recorded
    only inside the `Compaction` view, and so were missing from the history the log exists to keep. A first fix
    (longest surviving tail of the old view) failed on the real run: with a small retention window the compactor
    keeps none of the old tail. **Fix:** anchor on what `Compactor::compress_single_sequence` actually does. It
    splices exactly one summary where the evicted stretch began, so after the common prefix comes the summary,
    then the entries it kept from before, then new ones. The view ends at the last kept entry (or the summary), and
    everything after it is new `Message` events.
- **Proof:**
  - Domain: revisions don't count as compactions and the view stays exact; messages appended in the same snapshot
    as a compaction stay in the history; the same holds when the compaction keeps nothing; pre-compaction view
    replay is byte-exact (`replay_view_before_compaction`).
  - Writer: saves become messages plus a compaction, and the view equals the stored context; a resume from a cold
    cache adds no spurious compaction; seeding works; no context means nothing is written.
  - End to end: a compacting run has ≥ 1 compaction event, and each of `echo one` to `echo four` and the final
    answer is in the log **exactly once** as a message, although the working view summarised most of them away.
- **Limits:**
  - `replay_history` keeps each message as first recorded; revisions are not applied, since their index refers to
    the view.
  - The diff assumes peach's single-summary compaction shape. A future stage that rewrites differently (S0/S1
    stubs, T3.5/T3.6) must write its own events rather than rely on the diff. That is the point of recording
    compaction events at the source, and it is noted for those tasks.

## D-066 — T3.3: results a compaction summarises away get recall handles read with `read`, not a new tool (flagged) (2026-09-26)
- **Problem (R-CTX-3):** peach's summary keeps *that* a call ran (`**Execute:** cargo test`) but not what it returned.
  After compaction the only way back to a result is re-running the call, which is S2's "just re-run it" flaw and
  impossible for anything non-idempotent.
- **Decision: no new tool.** The spec names `recall(handle, start_line?, end_line?, pattern?)`. `read` already takes
  a path and a line range, and `fs_search` a pattern, so a handle that is a *file* gives the same four parameters
  through tools every model already uses well. A new tool would touch the catalog, descriptions, executor and every
  tool-definition snapshot, and add a tool to learn, against HACKATHON §21 ("more tools ≠ better") and principle 8.
  The artifact store (T3.1) stays the durable copy; the handle file is its readable face.
- **Built (`compaction_pipeline::recall`):**
  - When S3 runs, every tool result in the old view that is gone from the new one is written to a
    `peach_recall_*.txt` file.
  - The summary gains a **RECOVERABLE RESULTS** section: tool, call id, `read <path> (N lines)`, and the instruction
    to read or search it instead of re-running the call. That makes the withheld output loud (principle 4).
  - Handles are registered as dump files, so reading one counts as `offload_read`.
  - `PEACH_HARNESS_RECALL_HANDLES=1`, **default off**: it adds text to every summary and changes what the model is
    likely to do next, so it waits for an A/B (principle 6).
- **Bug found by the behaviour test:** `offload_read` stayed 0 even though the read happened. The hook registered
  the handles on `conversation.metrics`, but the orchestrator replaces those metrics wholesale with the tool
  context's copy every iteration, the same trap `compactions` already had a carve-out for. **Fix:**
  `TaskMetrics::absorb_dump_files` merges the hook's handles into the tool context's copy at that sync point.
- **Proof (R-CTX-3's acceptance):**
  - A scripted run echoes `MARKER_ONE`, and compaction summarises it away.
  - A `Turn::FromRequest` reads the handle named in the summary.
  - The next request holds `MARKER_ONE` again, and `offload_read ≥ 1`.
  - Without the flag, no section appears.
  - Unit tests: only results that left the view get handles with their full text; section format; the pipeline
    lists every handle below the summary.

## D-067 — T6.4: project memory is loaded; `memory_write` is deferred, because in a judged run it would edit the repo (2026-09-26)
- **Loaded:** `.peach/memory.md` (at the git root, else the working directory) joins the custom instructions after
  `AGENTS.md`, under a "Project memory" heading. It is bounded at 16,000 characters; a larger file is clipped with
  the shared loud sentence and the path to read the rest. There is no effect unless the file exists, so no flag.
  Unit test for the format and clipping; end to end, a marker in the file is in the model's first request.
- **Deferred, by decision:** R-MEM-1's `memory_write` tool. In the judged one-shot run it would write
  `.peach/memory.md` **into the repository under evaluation**, an unrelated change in the diff the panel scores
  (HACKATHON §24, "avoids unnecessary changes"), and a one-shot run has no later session to benefit. It would also
  add a catalog tool for every model to learn (§21). It belongs in interactive use, with approval, as the spec
  says. Revisit with T4.1's approval flow, and keep it disabled whenever the run is unattended.
- **T6.4 stays unticked**, with the load half done.

## D-068 — Where the brief stopped, and the exact commands to resume (2026-09-26, 17:10 UTC)
- **Stopped on a real blocker, as the brief allows:** steps 1–3 need paid models. At the last check the OpenRouter
  account still had **$0 credit**; the free allowance was spent (52/50, resets 00:00 UTC); the key **expires
  2026-09-27 17:46 UTC**. A background watcher polls for credit. Step 4's offline work went down TASKS in priority
  order: TH.1, TH.2, TH.3, TH.5, TH.7, T0.8, T1.1, T1.3, T3.1–T3.4, T3.7, T3.8 and T3.10 are ticked; T6.4 is half
  done by decision (D-053–D-067). What is left is live-run work or Tier 3.
- **Resume, in order** (`target/debug/peach` rebuilt at the current HEAD):
  1. Round 2, reachability and first ranking:
     `npx tsx benchmarks/hackathon/bakeoff.ts --label round2 --suite all --seeds 1 --bin "$PWD/target/debug/peach" --max-requests 60 --max-duration-secs 1500 --models "anthropic/claude-fable-5.1,anthropic/claude-opus-5.5,openai/gpt-6-sol,google/gemini-3.8-flash,x-ai/grok-4.7,~deepseek/deepseek-pro-latest,z-ai/glm-5.3,qwen/qwen3.8-max-0902,nvidia/nemotron-3-ultra-550b-a55b:free"`.
     Then `--seeds 3 --label round2-k3` on every model with at least one completed run. The default is the top of
     the report's ranking (success rate, then cost; blocked runs are excluded). Set it in
     `configuration/profiles/openrouter/peach.toml`.
  2. D-046 on the winner:
     `PEACH_SESSION__MODEL_ID=<winner> node benchmarks/hackathon/run.ts --agent peach --profile openrouter --bin "$PWD/target/debug/peach" --suite py-bugfix --max-requests 60 --max-duration-secs 1500`.
  3. One A/B per flag on the winner, k = 3, all six fixtures:
     `PEACH_SESSION__MODEL_ID=<winner> npx tsx benchmarks/hackathon/ab.ts --suite all --profile openrouter --seeds 3 --base "<FLAG>=0" --cand "<FLAG>=1" --label <flag>-<model> --estimate-inr-per-run 50 --yes`.
     The flags are `PEACH_HARNESS_COMPACT_TOOL_DOCS`, `_PARALLEL_READONLY`, `_TOOL_CORRECTION`, `_LINE_NUMBERS_OFF`,
     `_HANDOFF_NOTE` and `_RECALL_HANDLES`. The last two need tasks long enough to compact: add
     `PEACH_COMPACT__MESSAGE_THRESHOLD=12` to both arms. Flip a flag to default on only if success is not lower on
     both families. `offload_read` is now real (D-060), so the recovery column finally means something.

## D-069 — Cost stance reversed: free-tier models only; a new Makefile evaluation contract (2026-09-27)
- **Context:** D-050 ("open the field, cost is not the constraint") is withdrawn by the team. Direction now:
  "we don't have to spend money on API keys, so we want to go with the free version." This is not a return to
  D-025's ₹100 cap — it is stricter: no paid spend at all, free-tier only.
- **Consequence for the bake-off:** the round-2 plan in D-068 (9 models × 6 fixtures × k=3, ~$80–150) is
  cancelled as written. Scope down to free-tier-only models (`:free` on OpenRouter), most plausibly `nvidia/
  nemotron-3-ultra-550b-a55b:free` — already D-051's default, since it was the one model with a full passing
  record with no credit. Round 2 becomes: compare only the free-tier models against each other, within the
  50-requests/day account limit, spread across days if one day's allowance is insufficient. No frontier
  paid models (Opus, GPT, Gemini via OpenRouter) are in scope unless a future direction reverses this again.
- **The expiring key (D-068):** still needs replacing/regenerating before 2026-09-27 17:46 UTC regardless of
  this decision — a free-tier key still needs to exist and be valid. Regenerating a key costs nothing.
- **New organizer document:** `docs/harness/MAKEFILE_EVAL.md` records a Makefile-based evaluation contract
  supplied by the team (`~/Downloads/AI Harness Submission.md`), treated as authoritative alongside
  `HACKATHON.md` for submission mechanics. Requires a root `Makefile` (`setup`/`run`/`test`/`clean`), credential
  via a fixed `AI_API_KEY` env var (never hard-coded anywhere committed), text-only model (already true), and
  a model prescribed by the committee at evaluation time without source changes. See that file for the
  interpretation of "launch then supply the issue" (stdin + a `PROMPT=` override into the existing one-shot
  `peach exec` path) and how `AI_API_KEY` maps onto whichever profile variable is currently selected.

## D-070 — The root Makefile: the organisers' `setup`/`run`/`test`/`clean` contract over the existing entry point (2026-09-27)
- **Built (MAKEFILE_EVAL.md):** a root `Makefile` plus `harness/run-task`. No new execution mode: `make run`
  feeds the existing one-shot `harness/peach-ice-tea` → `peach exec` path.
  - `setup`: installs Rust with rustup and `protoc` with Homebrew when they are missing (otherwise it names the
    apt package and stops); release build via `harness/build.sh`; `npm ci`; the layout check.
  - `run`: the task comes from `PROMPT=` or stdin (piped, or typed and ended with Ctrl-D); both are the literal
    reading of "launch, then supply the issue".
  - `test`: the local hackathon suite, live.
  - `check`: offline checks with no key and no model: runner unit tests, reference solutions, peach's own
    integrity guard, and the layout check.
  - `clean`: `cargo clean` plus evidence and run reports.
- **`AI_API_KEY`:** exported as the variable the selected profile names (`PROFILE_KEY_VAR`), then unset, so it
  reaches peach only under that name. It is exported rather than passed as an argument, so it never appears in
  a process list. The wrapper strips every other provider key. A prescribed model is `MODEL=` (or `PROFILE=`),
  never a source change. `.env.example` holds the one empty variable.
- **Open question, handled defensively and flagged (like MAKEFILE_EVAL's stdin question):** which repository does
  the task apply to? `peach exec` works in its current directory, but `make run` runs inside the harness clone.
  `REPO=` names the target; it defaults to where `make` was invoked, so `make -f <harness>/Makefile run` works from
  inside the target. `make run` **refuses to run on the harness repository itself**, with the exact command to
  use instead, rather than silently letting the agent edit the harness. If the organisers' flow is different, it
  is a one-variable change.
- **`make test` spends model requests** (the suite is a live run); `make check` exists so a keyless evaluator
  still has something meaningful to run.
- **Verified from a clean clone** (HEAD plus exactly this commit's files, fresh `target/` and `node_modules`):
  - `make setup`: exit 0 (release build 5 min 42 s, 102 npm packages, layout ok).
  - `make run` from the harness directory is refused with the redirect message.
  - `make run REPO=<py-bugfix copy> TEST_COMMAND=… < issue.md`, with the issue on stdin: **`completed`, exit 0,
    64 s, 5 LLM calls** (66,960 input / 870 output tokens, 21,600 cached) on `nvidia/nemotron-3-ultra-550b-a55b:free`.
    The fixture's tests pass, integrity is clean (2 checked), the harness's own final run passed 4/0, the bundle
    is complete (11 files), the key string appears nowhere in the evidence, and no leftover credentials directory
    remains.
  - `make check`: 7/7 runner tests, 6/6 reference, 6/6 peach-cheat, layout ok. `make clean` removes `target/` and
    `evidence/`.
  - The run is also **D-046 reproduced on the current default model through the evaluator path**, at zero
    spend (free tier).

## D-071 — Free-tier round 2, day 1: a screen, not a ranking; a new default needs a full-suite result (2026-09-27)
- **Budget (D-069):** free models only; 50 requests/day per OpenRouter account. One six-fixture suite takes ~40–50
  requests (Nemotron Ultra: 40 in round 1), so a model can get one full suite per day at most. Day 1 therefore
  screens instead of ranking. **Spend: $0.**
- **Step 1, tool-call probe (1 request each):** 16 `:free` models support tools. Excluded: the incumbent, both
  `inkling` models (403, allow-listed apps only; not spoofed), the 2.6B model (too small, 65k context), and the
  two domain-tuned `ling` variants.
  - Pass: `cohere/north-mini-code`, `nvidia/nemotron-3-super-120b-a12b`, `nvidia/nemotron-3.5-lightning`,
    `dots-studio/dots-3-note-preview`, `poolside/laguna-s-2.1`, `nvidia/nemotron-3-nano-omni-30b-a3b-reasoning`.
  - Upstream 429 (retry another day, not a model result): `qwen/qwen3.8-27b`, `poolside/laguna-xs-2.1`,
    `google/gemma-4-31b-it`, `google/gemma-4-26b-a4b-it`.
- **Step 2, `py-bugfix` through the real harness** (`benchmarks/reports/models/20260926-free-screen-1.md`, 1 seed,
  capped at 7 requests):

  | Model | Result | Calls | Wall |
  |---|---|---|---|
  | dots-3-note-preview | ✓ | 4 | 43 s |
  | north-mini-code | ✓ | 6 | 46 s |
  | nemotron-3-super-120b | ✓ | 5 | 51 s |
  | laguna-s-2.1 | ✓ | 4 | 577 s |
  | nemotron-3-nano-omni-30b | ✗ request limit | 7 | 211 s |
  | nemotron-3.5-lightning | ✗ runner timeout (600 s) | 6 | 600 s |

  For comparison, the incumbent `nemotron-3-ultra` passed `py-bugfix` in 5 calls in round 1, and in 5 calls /
  64 s through `make run` today (D-070).
- **Decision:** the default stays `nemotron-3-ultra`, the only free model with a record across all six fixtures
  (D-051). `py-bugfix` is the easiest fixture, and one seed on it cannot unseat that. **Plan, one full suite per
  day:** dots-3-note-preview, then nemotron-3-super, north-mini-code and laguna-s (the last is slow, so its wall
  time counts against it). The four 429'd models get re-probed on a spare day. A challenger replaces the default
  only with a higher full-suite success rate, or an equal one with fewer calls (D-050's order, with cost replaced
  by calls since every model here is free).
- **Where it stops today:** ~3 free requests remain. Resume tomorrow with
  `OPENROUTER_API_KEY=… npx tsx benchmarks/hackathon/bakeoff.ts --label free-suite-dots --suite all --seeds 1 --bin "$PWD/target/debug/peach" --max-requests 50 --max-duration-secs 1500 --models dots-studio/dots-3-note-preview:free`.

## D-072 — MM.4: an exhausted quota or an outage fails over to the next model instead of ending the run (2026-09-27)
- **What broke:** D-040 made an exhausted quota fail *fast*, but it still ended the run, as did an outage that
  outlasted the retries. On a free tier, per-model upstream rate limits are routine (4 of 10 models on D-071's
  probe), so one busy model could cost the whole one-shot task.
- **Built:**
  - `model_failover::failover_reason` qualifies exactly two cases: a quota D-040 identifies as unrecoverable, and a
    `Retryable` error (rate limit, 5xx, transport) that survived every retry. Anything else, such as a 400 for a
    malformed request, would fail the same way on another model and still ends the run.
  - `Failover` holds `PEACH_HARNESS_FALLBACK_MODELS` (same provider, in order, without the current model or
    duplicates). In `orch.rs` the failing iteration counts the failed call on the tool context's metrics (the copy
    the sync keeps), emits `recovery{action: model_failover, trigger: "<old>: <reason>; continuing on <new>"}`,
    switches `model_id` and `agent.model`, and runs the iteration again.
  - With no list, nothing changes (principle 5).
- **Profile:** `openrouter` falls back to nemotron-3-super, dots-3-note-preview and north-mini-code, the models that
  passed D-071's screen. **Limit:** OpenRouter's 50 free requests/day are per *account*, so failover cannot rescue a
  run once that allowance is spent. It covers per-model upstream limits and outages.
- **Proof (real binary, scripted provider):**
  - No-credit 402: request 1 goes to `scripted-model`, request 2 to `fallback-model`; the run completes (exit 0) with
    `failed_llm_calls` 1 and the recovery event.
  - Three 503s with two retries: the fourth request goes to `fallback-model`, and the run completes.
  - A 400 does not fail over (exit 1, one request).
  - The existing D-040 test (no list) still fails fast after one request.

## D-073 — T1.4/T1.5: content-aware shell shaping built behind two flags; a path-and-full-stop ambiguity fixed in every notice (2026-09-27)
- **Built (R-OUT-2), each behind its own default-off flag so each gets its own A/B:**
  - `truncation::classify` (T1.4, `PEACH_HARNESS_SEARCH_REGROUP=1`) classifies a command by the first program of
    each pipeline/list segment, skipping env assignments and `sudo`/`time`/`env`, with known subcommands
    (`git grep|diff|log|show|blame`, `python -m pytest|unittest|pip…`) as Search, Noise, SourceLike or Unknown.
    Search wins over noise, which wins over source-like: `cargo test | grep FAIL` is search. It regroups
    `path:line:text` search output under one header per file, dropping only the repeated prefix and never a
    match, and only when the result is smaller.
  - `truncation::compress_noise` (T1.5, `PEACH_HARNESS_NOISE_COMPRESSION=1`) handles builds, installs, test runners
    and linters:
    - strips ANSI and carriage-return progress frames;
    - always keeps signal lines (error, warn, fail, panic, exception, traceback, assert, fatal, denied, not found)
      with ±3 lines of context, plus the last 15 lines (the summary);
    - counts passing-test lines (cargo/pytest/jest/go/unittest shapes);
    - collapses runs of 3 or more lines that match once digits are blurred ("… N similar lines");
    - applies only if that saves at least 30% and 2,000 characters.
  - It is hooked into the `Shell` branch of `operation.rs` before head/tail truncation. `dump_operation` saves the
    raw stream whenever shaping withholds anything, even under the caps, and the output ends with the shared
    recovery sentence (R-OUT-3/4), so a read of it counts as `offload_read`.
- **Found while testing:** the new notice ended "…read /path/file.txt." A full stop glued to a path is read as part
  of it: my own end-to-end test's reader did, and a model can too, which means a failed `read` and a lost turn.
  Three live notices from T1.1 (shell line-clipping, fetch, MCP) had the same shape. **All now close with a
  parenthetical**: "Full output: read /path (the complete output)." The only snapshot change is exactly that one
  line (`net_fetch_truncated`).
- **Proof:**
  - Unit tests with realistic logs: a 300-line cargo build keeps the E0308 error and "could not compile", and
    shrinks more than 5×; a 200-test pytest run keeps the FAILED line, the AssertionError and the summary, and
    counts the passes; npm progress frames and ANSI are stripped while "npm WARN" is kept; short or all-signal
    output passes through. Classification over 10 command shapes; lossless regrouping with every match
    re-checked.
  - End to end: `./make` prints 300 `Compiling` lines and an error. With the flag, the model sees the error and the
    recovery sentence, not the noise; reading the path brings the full output back, and `offload_read == 1`.
    Without the flag, the output is untouched.
- **Ship bar (R-OUT-2):** the A/B must show success within noise **and the noise class's recovery rate under 2%**,
  now measurable because `offload_read` is real (D-060). **T1.4 and T1.5 stay unticked** until then.

## D-074 — T3.6: S1 offload runs before the summary and can make it unnecessary; two things fixed on the way (2026-09-27)
- **Built (R-CTX-2 S1, `PEACH_HARNESS_OFFLOAD=1`, default off):** `compaction_pipeline::offload`. Outside the
  retention window, every tool result of at least 2,000 characters becomes a stub. The stub carries a marker,
  size in characters and lines, the tool, an error flag, the first 300 characters, and "Full result: read <handle>
  (the complete output)". Calls and results stay paired, and user and assistant text is untouched. The operation
  is idempotent. Handles are registered as dump files, so reading one counts as `offload_read`.
- **Fixed: the pipeline always ran its final stage.** T3.4's loop ran the last stage "if reached", so with S1 in
  front, the lossy S3 would still run every time, defeating S1. The rule is now "stop as soon as the context is at
  or below target", with the first stage always running, so a one-stage pipeline still behaves exactly as before
  (the golden test still passes). The hook's target is ¾ of `token_threshold` when tokens triggered compaction
  (a margin, so the next turn does not re-trigger) and 0 when a message or turn count did, so S3 still runs to
  reduce the count.
- **Fixed ahead of time: the event log would have misread S1.** D-065's diff understood peach's single-summary
  compaction. An in-place rewrite would have logged each stub as a new message. `events_between` now recognises
  "same positions, each entry unchanged or an offload stub (`OFFLOAD_STUB_MARKER`)", plus new messages after it,
  as one compaction followed by messages (domain test).
- **Where S1 matters, learned from the test:** shell output is already clipped to 100 + 100 lines before it
  reaches the context (about 1.2 KB for `seq 1 3000`), so S1 rarely touches shell results. It pays for itself on
  `read` and `fetch` results.
- **Proof:**
  - Unit: only large results outside the window become stubs with readable handles; the window, small results and
    user text are untouched; running it twice changes nothing.
  - Pipeline: when S1 brings the context under target, `stage_reached == "offload"` and no message is summarised
    away.
  - End to end: a 16 KB `read`, then compaction on a 7,000-token threshold. The stub reaches the model, the bulk is
    gone from context, **no summary runs**, reading the handle restores the text, and `offload_read == 1`.
- **T3.6 stays unticked** (`[A/B]`: long tasks, with success and total input tokens against flag off).

## D-075 — T3.5: S0 supersede; the end-to-end test caught it stubbing the newest result when call ids repeat (2026-09-27)
- **Built (R-CTX-2 S0, `PEACH_HARNESS_SUPERSEDE=1`, default off; it runs first, then S1, then S3):**
  `compaction_pipeline::supersede`. Deterministic, with no model. Outside the retention window, a result becomes a
  stub naming what superseded it, with the full text one `read` away, in two cases:
  - a `read` of a file that was later read again, or changed by `write`, `patch`, `multi_patch`, `remove` or
    `undo`;
  - a shell command later re-run verbatim.

  Stubs carry the offload marker, so the event log treats them as an in-place rewrite (D-074).
- **Not built, by decision:** R-CTX-2's "search superseded by a narrower search in the same path". Deciding that one
  search is narrower than another (pattern subsumption, path containment, flags) needs a judgement that a wrong
  answer turns into silently hiding live matches, which is lossy in the worst way. It can be revisited with the
  relevance scorer (T3.9).
- **Bug found by the end-to-end test:** the first version keyed staleness by call id. The scripted provider reuses
  `call_1` for every call, and some OpenAI-compatible gateways reuse simple ids across turns too, so marking the
  first read's id stale also stubbed **the newest read**. The model would have lost the current file entirely,
  with a stub pointing at an old copy. **Fix:** each result is paired with the most recent unanswered call of the
  same id, by occurrence, and staleness is decided per result position. A unit test pins the reused-id case: the
  newest result is never stubbed.
- **Proof:**
  - Unit: stale by re-read, by edit, and by re-run; untouched files and the newest run are kept; the retention
    window is untouched; the reused-id case.
  - End to end: a 16 KB file read twice, compaction on tokens. The first read reaches the model as "big.txt was
    read again later", the newest read's content is intact in the last request, and **no summary runs**. While
    tuning, thresholds where S0 alone was not enough correctly fell through to S3, which is the D-074 stop rule
    working as intended.
- **T3.5 stays unticked** (`[A/B]`).

## D-076 — T3.11: a soft trigger that compacts reversibly and early, and the cache cost of each compaction in the report (2026-09-27)
- **R-CTX-7, soft trigger (`PEACH_HARNESS_SOFT_COMPACTION=1`, default off):**
  - When the hard trigger has not fired but tokens reach ¾ of `token_threshold` (the spec's 0.6 of the window
    against the hard trigger's 0.8), the hook runs `Pipeline::reversible`: S0 supersede, then S1 offload. **It
    never runs the lossy summary.**
  - It aims at ¾ of the soft threshold. It records a compaction (metrics and telemetry) only when it actually
    changed the context, and registers its handles for `offload_read`.
  - This keeps compaction deliberate: large, reversible cuts early, and the prefix-invalidating summary rarely.
  - The soft pass enables S0/S1 by itself, independent of their own flags, which govern the hard pass.
- **R-CTX-8, cache accounting:**
  - Per-call cached tokens were already recorded (D-032, D-036). The report now adds
    `context.cache_around_compactions`: the cache hit rate (`cached / input`) of the model call just before and
    just after each compaction, which shows what a compaction cost the prompt cache. Markdown: "Cache hit rate
    around each compaction (call before → call after): 85%→89%".
  - Serialised only when non-empty. The golden snapshot changed by exactly that field for the fixture's one
    compaction.
  - "Keep the system prompt and tool definitions first and stable" already holds: nothing in the pipeline touches
    them, and S0–S2 edit only tool results.
- **Proof:**
  - Unit: the soft pipeline never summarises.
  - End to end: two 16 KB reads under a 15,000-token hard threshold. With the flag, the soft trigger stubs the
    first read ("read again later"), no summary runs, and `context_compaction` is in telemetry. **Control:** the
    same run without the flag does not compact at all, so the soft trigger did it. The end-to-end test was stable
    across hard thresholds from 13,000 to 16,000.
- **T3.11 stays unticked** (`[A/B]`: long tasks; success, total input tokens, compaction count, and the cache rate
  around compactions, flag on vs off).

## D-077 — T3.9 in part: S2 relevance scoring wired with the heuristic scorer; `LlmScorer` deferred under the free-tier stance (2026-09-27)
- **Built (R-CTX-2 S2, `PEACH_HARNESS_SCORE_STAGE=1`, default off; order S0 → S1 → S2 → S3):**
  `compaction_pipeline::score`.
  - Every non-stub tool result gets a summary for the scorer: tool, redacted input preview of at most 200
    characters, status, size, position, and `referenced_later` computed from the user and assistant text after it.
  - T3.8's `build_plan` then applies the fail-open rules (pinning, redaction, keep-on-error).
  - The decision is applied: keep; truncate to the head plus a handle; or drop to a stub with a handle. The call is
    never removed, so calls and results stay paired.
  - Stubs carry the offload marker, so the event log reads them as an in-place rewrite (D-074).
  - Results get a synthetic id from their position (`m<index>`), because provider call ids can repeat and keying
    by id stubbed the wrong result in D-075.
- **Scorer choice, by decision:** the heuristic scorer (T3.8, no model). R-CTX-4's `LlmScorer` asks the compaction
  model two questions per call. That is at least one extra request per compaction, and under D-069 (free tier,
  50 requests a day across the account) it competes directly with the runs. The compaction hook also has no model
  services today, so it would need new plumbing. `LlmScorer` stays deferred, and **T3.9 stays unticked**. The
  stage takes any `RelevanceScorer`, so `LlmScorer` drops in without touching the pipeline.
- **Proof:**
  - Unit: an old unreferenced 20 kB read is cut while one whose file the assistant named later is kept (T3.8's
    `referenced_later` doing its job); a second scoring pass changes nothing.
  - End to end: a 16 KB read the conversation never mentions again, compaction on tokens. S2 cuts it, and **no
    summary runs**. Stable at 6,000–7,000; 8,000 never triggers.

## D-078 — T0.7: behavioural checks over real runs; on 19 real bundles the one failure is a run that ran out of requests (2026-09-27)
- **Built (R-EVAL-4):** `benchmarks/hackathon/behaviour.ts`. Four checks over an evidence bundle's
  `telemetry.jsonl`, from what the agent *did*, each pass / fail / n/a (n/a when the run gave it nothing to judge):
  - `read_before_patch`: every edit of an existing file came after a read of it; creating a file needs none;
  - `verified_after_last_edit`: a green *agent* test run came after the last edit (the harness's own final run does
    not count);
  - `truncation_awareness`: withheld output was recovered via its handle, not by re-running the same command blind;
  - `todo_usage`: work that edits two or more files used `todo_write`.

  CLI: `node benchmarks/hackathon/behaviour.ts <evidence-dir>…`; exits 1 on any failure.
- **Two checker flaws, found by running it on real bundles before trusting it:**
  - It counted *refused* edits as edits. Nemotron 3.5 Lightning patched a path outside the task repository without
    reading it, and peach's own read-before-edit guard refused the call. The run looked like a behaviour failure,
    but the runtime had enforced the rule, which is principle 3 working. Refused attempts are now excluded and
    reported in the detail.
  - Telemetry truncates arguments at 2,000 characters, so a large `write` has unparseable JSON and lost both its
    path and its `overwrite` flag. `file_path` and `overwrite` are now recovered from the raw text.
- **Real evidence** (`benchmarks/reports/behaviour/2026-09-27-real-runs.md`, all 19 bundles from real model runs
  so far: the `make run` verification, the free-tier screen and round 1):
  - Every successful run passes every applicable check.
  - The single failure is `verified_after_last_edit` on Nemotron Nano Omni's `py-bugfix`, which hit its 7-request
    cap after patching and before testing. That is a true negative.
  - `truncation_awareness` was n/a everywhere: none of these runs produced withheld output. The scripted
    end-to-end tests (D-060, D-073) cover that path mechanically.
- **Scope note:** R-EVAL-4's "parallel subagents" check is not included. No real run has used a subagent yet, so
  there is nothing to validate a check against. It moves to T3.13 with the other behaviours that need specific
  tasks.
- **T0.7 ticked:** green on the baseline (every run that completed passes), with the one true negative explained.

## D-079 — T0.9: tool micro-evals for the tools this project changed, and a per-tool error-rate report from real runs (2026-09-27)
- **Micro-evals (R-EVAL-3), `benchmarks/evals/tool_<name>/task.yml`**, in the existing eval format, one per tool this
  project changed, each isolating one failure class with its feature flag on:
  - `tool_shell` (T1.5 compression, wrong sequence): find and fix the one error in a 300-line build without a blind
    re-run; the recovery sentence must be present; no shell tool errors.
  - `tool_read` (T1.2 line numbers off, wrong args): the read comes before the patch, and no patch fails to match
    (for example a search text carrying `  3:` prefixes).
  - `tool_write` (T2.6 correction, wrong args): the file is written, and no write is rejected for unknown or
    missing fields.

  They run over `benchmarks/models.free.csv` (Nemotron Ultra and the three fallbacks; D-069), not the paid default
  list. **They have not been run yet:** each run spends free requests, and today's went to the bake-off.
- **Per-tool error rate by model:** `benchmarks/hackathon/tool_errors.ts` reads run reports (peach's own
  `tool_calls` / `tool_errors`). It takes the model from each run's **telemetry** (`model_call.model`), because the
  report files do not record it and file-name parsing proved brittle. Over every real run so far
  (`benchmarks/reports/tools/2026-09-27-tool-errors.md`), the default model has **0 errors in 67 tool calls**.
  Lightning's 1/2 patch errors is the call peach's read-before-edit guard refused (D-078), and DeepSeek Pro had one
  failed read in 13.
- **Not built: the CI job.** A CI job that runs live micro-evals needs a provider key in CI and would spend the
  50 free requests a day. The report script is the CI-ready half. **T0.9 stays unticked** until the micro-evals
  have run on the free models and the CI question is decided (team: whether to store a free key as a CI secret).

## D-080 — ARCHITECTURE's §25 answers now cite a real run, committed as evidence (2026-09-27)
- **Gap (D-044):** `documentation/ARCHITECTURE.md` answered HACKATHON §25 with code paths and decision records, but its
  telemetry references were field names proven by scripted tests. It also predated everything since D-048: any-model,
  the compaction pipeline, the event log, failover and the Makefile.
- **Now:** every answer cites what a real run shows. That run's bundle is committed at
  `documentation/evidence/2026-09-27-make-run-py-bugfix/`: the D-070 clean-clone `make run` on the default free model
  (132 KB, no key-shaped strings). Checked facts it cites:
  - the prompt composition (tool definitions 9,497 of 12,636 tokens);
  - `origin_call_id` links back to the model call that requested each tool;
  - the agent's green `test_run`, then the harness's own final run (4/0);
  - `cached_tokens` 0, 0, 4,320, 8,640, 8,640 (a 32% hit rate);
  - a **real recovery**: `pytest` missing → `test_run {failure_class: environment}` + `recovery_hint` → the next call
    used `unittest` → green;
  - the integrity verify of 2 files.

  A script re-derived each cited number from the committed telemetry before commit.
- **Honest gaps stated in the document:** no real run has compacted or failed over yet. Those answers name the
  end-to-end tests that prove the mechanisms with a scripted model. TH.9's "grounded in real telemetry" note is closed.

## D-081 — Handoff brief Tier 0: the bare evaluator recipe now authenticates any provider; Linux `protoc`; failover visible and opt-in (2026-09-27)
Source: `docs/harness/AGENT_HANDOFF_BRIEF.md`, an audit pass supplied by the team.
- **0.1, profile selection. The brief's premise conflicted with D-049/D-069.** The brief read HACKATHON.md's
  "Gemini 3.8 High" as fixed and asked for `PROFILE ?= gemini`. The team had since allowed any model and chosen the
  OpenRouter free tier. Either hard-coded default breaks one of the two cases, while the brief's real concern
  is sound: the organisers' literal recipe (`export AI_API_KEY; make setup; make run`, no `PROFILE`) must
  authenticate. **Decision:** `harness/select-profile`, shared by `make run` and `make test`, picks the profile
  from the key's shape.
  - `sk-or-…` → `openrouter`; `nvapi-…` → `nvidia-deepseek`.
  - `AIza…` (Google AI Studio) **and anything unrecognised → `gemini`**, the model HACKATHON.md names.
  - An explicit `PROFILE=` always wins.
  - **Proven live:** only `AI_API_KEY` set to the AI Studio key, and a bare `make run` on a fresh `py-bugfix` copy.
    The run selected `gemini`, authenticated, and made 18 calls on `gemini-3.8-flash`. **Gemini fixed the bug**:
    the agent's test run and the harness's final run both passed. It then spent its remaining requests on
    `todo_write` bookkeeping until the free tier's 20 requests/day ran out; the outcome is `error`, named by
    D-040's detector. The key appears nowhere in the evidence. Bundle committed at
    `documentation/evidence/2026-09-27-make-run-gemini-py-bugfix/`. The todo-spending pattern matches D-032, and
    `PEACH_HARNESS_COMPACT_TOOL_DOCS` (which drops the "use todos proactively" examples) is the natural A/B for it.
- **0.2, Linux `protoc`:** the brief's `make setup` hunks are applied as given. When there is no brew, it runs
  `apt-get` non-interactively (as root or with passwordless sudo). Otherwise it downloads the pinned protoc 36.2
  release into `.tools/protoc` (gitignored), which the build then uses. **Checked here:** make parses the recipe,
  the protoc block is valid shell, both release URLs (x86_64, aarch_64) return 200, and the archive has
  `bin/protoc` + `include/`. **Correction to the brief:** `file` shows the Linux binary is *statically* linked, so
  it runs on musl (Alpine) too; DEV.md says so. **Not run on Linux:** no Docker daemon was available.
- **0.3, failover visibility and default:**
  - The report's `error_recovery.model_failover_count` counts `recovery: model_failover`, and the markdown flags
    it when non-zero.
  - A scripted test runs the same task twice, plain and with a no-credit 402 then a fallback. **Outcome and test
    result are identical**, with counts 0 and 1, so scoring is model-agnostic.
  - **Past data checked:** all 19 real bundles contain exactly one model in their `model_call` events. My first
    check wrongly counted retry events, whose `operation` field is `"model_call"`. No earlier comparison was
    contaminated; the free screen predates MM.4.
  - **MM.4 is now opt-in** (`openrouter/profile.env`'s fallback list commented out). It shipped on with scripted
    proof only, not an A/B (principle 6). This supersedes D-072's "on in the profile".
- **Tests:** `cargo insta test --workspace`: 3021 passed. The known timing test
  (`test_concurrent_operations_dont_block_runtime`, D-031) failed once under load and passed 5/5 alone and on the
  next full run.

## D-082 — Brief Tier 1.1: an enforced doom-loop escalation ladder (R-LOOP-5), behind a flag (2026-09-27)
- **Built as the brief specified, with two changes:**
  - `peach_app::doom_loop_escalation::EscalationGuard` fingerprints each call by name and canonicalised arguments.
    The 1st repeat runs with a warning appended, the 2nd repeat is withheld and answered with the warning, and the
    3rd repeat is withheld and pauses the run. After a pause, a one-shot re-arm lets that call through once, so an
    End hook that continues the run does not re-trigger the pause at once.
  - Warning templates: `peach-doom-loop-{warn,skip,pause}.md`.
  - `InterruptionReason::DoomLoopEscalation` and exec outcome `doom_loop_escalation` (exit 6), mapped in both
    `ui.rs` matches.
  - `hooks/doom_loop.rs` is untouched: upstream's nudge also catches `[A,B,C]` cycles, which this does not.
  - **Change 1, per-orchestrator setting.** The brief read the flag inside `execute_tool_calls`. A process-wide env
    read would leak between unit tests running in parallel, including upstream's doom-loop spec with its four
    identical calls. So `Orchestrator.doom_loop_escalation` is a setting, like `parallel_readonly`: `app.rs` sets it
    from `PEACH_HARNESS_DOOM_LOOP_ESCALATION`, and tests set it directly.
  - **Change 2, borrow fix.** The brief's `ui.rs` arm moved `tool_name` out of `reason`, which is used afterwards
    (a compile error); the title match now matches on a reference.
- **Proof:**
  - The brief's 4 unit tests.
  - An orchestrator spec: four identical calls give 4 results, **2 executed**, 1 warned, and an `Interrupt` with
    `occurrences: 4`.
  - An `exec` run through the real binary with the env flag: **exit 6**, outcome `doom_loop_escalation`, and the
    run stops after the fourth request.
  - Upstream's doom-loop tests are unchanged and pass.
- **SPEC:** R-LOOP-5 added to §3. **TASKS:** T2.7, `[A/B]`, default off.

## D-083 — Brief Tier 1.2: a hard runtime verification gate (R-HACK-10), behind a config flag (2026-09-27)
- **Built as the brief specified:** config `runtime_verify_gate` (default `false`, in `.peach.toml` and the schema;
  env `PEACH_RUNTIME_VERIFY_GATE=true`) and `hooks/runtime_verify_gate.rs`.
  - On a **voluntary** stop in an unattended run with a known test command, the harness runs that command itself
    (`verify::run_gate`, `test_run.origin: harness_runtime_gate`, 180 s timeout).
  - It then re-verifies and restores protected files via the new `runtime::IntegrityHandle` /
    `restore_integrity_if_installed`, the same guarantee the final run gives.
  - On failure, it appends the command's **real** output (class, exit code, last lines, recovery hint) as the next
    user turn, so the loop continues. `orch.rs` is unchanged: the End-hook extension point already does this.
  - Capped at `MAX_RUNTIME_GATE_ATTEMPTS = 2`; after that it records `agent_state: runtime_verify_gate_exhausted`
    and lets the run end.
  - When on, it **replaces** the soft `VerifyGateHandler` (D-037) instead of stacking, to avoid double messaging.
  - `verify::run_final` now shares `run_test_command` with `run_gate`; the final run's behaviour is unchanged.
- **Changed from the brief:** its first end-to-end test expected a `harness_final` test run but passed no
  `--evidence-dir`. The harness runs its final test only when writing an evidence bundle, as every judged run does.
  The test now passes `--evidence-dir`, and its assertions are otherwise as specified.
- **Proof:** three end-to-end tests through the real binary:
  - A premature "Done." on unfixed code is sent back with the real `test_assertion` output; after the fix the test
    runs read gate `test_assertion` → gate `passed` → final `passed`, with 4 requests and the file fixed.
  - With the flag on but no test command, the gate fails open: one request, `completed`.
  - With the flag off, it is inert.
- **TASKS:** TH.10, `[A/B]` against the soft gate (success, LLM calls, wall time, recovery rate).

## D-084 — Brief Tier 2 (offline part): a cache breakpoint on the Anthropic tool array, with the 4-marker limit enforced (2026-09-27)
- **Built:** `SetCache` now marks the **last** tool definition as a cache breakpoint. One marker at the end of the
  static tool array caches the whole array (about 38 KB, D-039) on its own, instead of only as part of the system
  prefix. Tool markers are cleared first, so exactly one tool carries one. `count_cache_breakpoints` counts markers
  across system messages, message content and tools. The tool marker is added **only if the total stays within
  Anthropic's 4** (it rejects more with a 400); the brief's version lacked that guard. There is no flag: this sets a
  field that was previously never set, and does not change what the model sees (principle 6 applies to behaviour).
- **Pre-existing upstream issue found, recorded and not changed:** upstream marks *every* system message plus the
  last message. With 4 system messages that is already 5 markers, which Anthropic would reject. Peach sends at most
  3 (2 normally, 3 under OAuth), so this does not occur today. Capping upstream's system markers would change
  upstream caching, so it is left alone.
- **Tests:**
  - only the last of 5 tools gets a marker;
  - for every shape peach sends (0–3 system messages × short, medium and long conversations × 12 tools), 4 markers
    or fewer;
  - with 3 system messages the limit is already reached and no tool marker is added;
  - with 4 system messages the tool marker never adds to upstream's 5.
- Gemini's unused `Part.cache_control` gets a comment explaining why it stays unset (Gemini's caching is implicit).
  Bedrock's equivalent was not changed (not on the judged path).

## D-085 — NVIDIA NIM as a second free request pool; compact tool docs shipped on after A/Bs on two families (2026-09-27)
- **The NIM pool (development only, never the judged run):** the team supplied NVIDIA NIM keys; one tool-call probe
  each succeeded for `nvidia/nemotron-3-ultra-550b-a55b`, `z-ai/glm-5.3`, `moonshotai/kimi-k3`,
  `nvidia/nemotron-3.5-lightning-30b-a3b`, `meta/muse-glimmer-30b` and `google/gemma-4-31b-it` (the Llama vision key
  was skipped, per the team).
  - Keys are used only through each command's `NVIDIA_API_KEY` and written to no file (CLAUDE.md). They were
    pasted into the chat, so they must be rotated after the project.
  - **Shared quota: unknown.** NIM responses carry only `nvcf-reqid`, with no rate-limit or account headers, so
    runs are spread across keys and a 429 would be the answer.
  - New profile `configuration/profiles/nvidia-nim` (role `development`) registers all six on the generic `nvidia`
    provider; `PEACH_SESSION__MODEL_ID` picks one.
- **Choosing the second family (smoke on `py-bugfix` and `node-feature`):**
  - Nemotron Ultra on NIM: `py-bugfix` in 5 calls and 21 s.
  - GLM 5.3: 2/2, but about 7 minutes per fixture.
  - Kimi K3 **declared done with tests failing** after 2 calls on `py-bugfix`.
  - Muse Glimmer declared done after **one call and no edit** on `node-feature`.

  The soft gate (D-037) cannot catch a stop with no edit. That is TH.10's case, so the TH.10 A/B runs on Kimi.
- **Compact tool docs (D-039) A/B on NIM Nemotron Ultra, all 6 fixtures × 2 seeds**
  (`benchmarks/reports/ab/2026-09-26-compact-tool-docs-nim-ultra.md`): success **12/12 vs 12/12**, input tokens
  **−6.6%** per run, output −6.4%, LLM calls +4.4% (within noise: one fixture's two seeds took 14 and 25 calls in
  the same arm), wall time +0.8%. With the DeepSeek A/B (`2026-09-25-compact-tool-docs-nim.md`: 100%/100%, −17.6%
  input, −4% calls), that is **two model families with no success loss and fewer input tokens: principle 6's bar is
  met.**
- **Shipped default-on:** `tool_docs::enabled()` is on unless `PEACH_HARNESS_COMPACT_TOOL_DOCS=0`. A new test pins
  the default (no `<example` in the tools of a default run); the existing test still compares off/on explicitly.
  **The graded model is checked next:** the team asked for a Gemini arm because the D-081 run spent its 20
  requests partly on `todo_write`. That runs when Gemini's daily quota resets, and a regression there reverts this.
- **Runner fix:** `ab.ts` counted `--suite all` as one fixture in its summary line (cosmetic; every fixture ran).

## D-086 — Brief Tier 3.1: each recovery event says whose failure it answered (2026-09-27)
- **Why:** a report that says "3 recoveries" does not say whether the harness should change or the model is
  weak. The A/B and bake-off reports need that split to put a regression on the right side.
- **What:** `Recovery` gains an optional `attribution` (`model` / `harness` / `environment` / `ambiguous`),
  omitted when unknown, so older logs still parse. The telemetry schema moves to **0.2.0**. Each emitter tags its
  own events:

  | Action | Attribution | Why |
  |---|---|---|
  | `model_failover`, `subagent_model_failed` | harness | the harness chose the model that failed |
  | `tool_argument_renamed` | model | the model misnamed an argument |
  | `recovery_hint` for an environment failure | environment | a runner or module is missing |
  | `recovery_hint` for a compile failure | model | its code does not build |
  | `recovery_hint` for a timeout | ambiguous | an infinite loop, or a slow machine |
  | `offload_read` | harness | recovering output the harness withheld |
  | `reread_same_range`, `rerun_same_command` | model | the model repeating itself |

- **The D-060 counters are now events too.** `record_read` and `record_shell_command` return which counter fired,
  and `tool_executor` emits it. Before this, the counters existed only as totals in `exec.json`, with no position
  in the timeline. Redaction still applies (the sink redacts free text, so a shell command's secrets never reach
  disk).
- **Report:** `error_recovery.recoveries_by_attribution` shows the tally, and the markdown gains a
  "Recoveries by cause" line. The scripted-model failover test checks the tally (`{"harness": 1}`).
- No change to what the model sees, so no A/B is needed.

## D-087 — Brief Tier 3.2: `write_note`, a scratchpad kept outside the context (R-CTX-10, T3.14) (2026-09-27)
- **What:** a new tool, `write_note` (one flat, required `note`), stores notes on `Metrics.notes`, never in
  `Context.messages`, so no compaction stage (S0–S3) can see or drop them.
  - Each note gets a permanent label, `[note N]`.
  - Notes are redacted (`redact::redact`), cut at 500 characters (with a marker saying so) and capped at 20; the
    oldest is evicted first.
  - Every note is appended to the event log as `ThreadEvent::Note` (tracked by highest id, so a resumed process
    never logs one twice), so an evicted note is still recoverable.
  - Notes also persist in the conversation's metrics record, so a resumed conversation keeps them. Upstream does
    not persist todos this way.
- **Departure from the brief (re-surfacing rule):** the brief re-appends the scratchpad "if the notes changed
  since the last reminder". That would repeat every note right after the `write_note` result that already shows
  it, costing tokens every time a note is written (principle 1). Instead, `NotesHandler` re-appends all notes,
  verbatim, only when some note's label is **no longer in view**, meaning a summary replaced both its tool result
  and any earlier reminder.
  - A note therefore costs its tokens once while it is visible, and comes back only when compaction took it away.
  - The hook runs on the response hook after `CompactionHandler`, so the reminder reaches the very next request.
    The brief suggested a per-turn hook, but a request-hook injection lands one request late (orch syncs
    `self.conversation.context` into the local `context` only after the response hook). The upstream doom-loop
    reminder has the same one-request lag; it is left alone.
- **Gating:** the tool is filtered out of the system tools, and refused at execution, unless
  `PEACH_HARNESS_WRITE_NOTE=1` (principle 6). Without the flag the model sees nothing new, and the
  rendered-tool-descriptions snapshot is unchanged. The catalog-wide snapshots (Gemini declarations, OpenAI
  Responses, definition JSON) gain the tool, and its schema is flat with `required` (Gemini-compatible).
- **Tests:**
  - `scratchpad.rs`: ids, eviction, cut.
  - `notes.rs`: in-view notes are not repeated; out-of-view notes come back verbatim once; the reminder itself
    keeps them in view.
  - `writer.rs`: logged once across a resume.
  - End to end with the scripted model: a note written before compaction reaches the last request verbatim and is
    in `thread_events`, and the tool is absent by default.
- **A/B:** T3.14, pending. Like T3.10, it needs runs long enough to compact, so both arms use a low
  `PEACH_COMPACT__MESSAGE_THRESHOLD`.

## D-088 — The runtime verification gate ships on by default (TH.10, R-HACK-10) (2026-09-27)
- **A/B, 6 fixtures × 1 seed per family, on the NIM pool (development only):**

  | Model (family) | Success base → cand | Input tokens | LLM calls | Wall time | Report |
  |---|---|---|---|---|---|
  | Kimi K3 (Moonshot) | **0/6 → 5/6** | +224% | +191% | +220% | `2026-09-26-runtime-verify-gate-nim-kimi.md` |
  | Muse Glimmer 30B (Meta) | 6/6 → 6/6 | −2.1% | ±0% | −11.6% | `2026-09-26-runtime-verify-gate-nim-muse.md` |

  - **Kimi (base):** in 5 of 6 runs it declared done after one call without fixing anything (D-085 saw the same).
  - **Kimi (candidate):** the gate ran in every run and sent the real failure back. Four runs then succeeded; in
    three of them the gate had to run a second time. `node-feature` used both attempts and still failed, and the
    cap let it end instead of looping.
  - **Kimi's cost:** tokens went up because the runs now do the task. Principle 1 optimises the completed task,
    so paying more to succeed where the base produced nothing is a gain.
  - **Muse:** the gate ran once per run, passed each time, and added no model call.
- **Principle 6 is met:** two families, no success loss, one large gain, and no cost regression on the model that
  already succeeds. `runtime_verify_gate = true` in `.peach.toml`; `PEACH_RUNTIME_VERIFY_GATE=false` restores the
  soft gate (D-037). The gate still fails open with no exec runtime or no test command, so interactive use is
  unchanged.
- **Tests:** the scripted-model helper pins the gate off as a baseline. Those conversations model a stop being
  accepted; with the gate on, their unfixed fixtures would draw extra turns they do not script. Two things keep
  the gate covered:
  - The gate's own tests set it explicitly.
  - New `test_the_hard_gate_is_on_by_default` unsets the variable to check the shipped default.

  No assertion was weakened. Separately, `make check`'s `peach-cheat` integrity run passed 6/6 on the new binary:
  it never reaches a voluntary stop, so the gate is not involved.
- **Caveat:** 1 seed per family, so the confidence intervals are wide (Kimi's candidate, 44–97%). The size of the
  Kimi effect (0 → 5) and its mechanism, visible in every bundle, carry the decision. The graded Gemini model is
  not measured yet; a Gemini arm joins the compact-docs check at the next quota reset.

## D-089 — Brief Tier 3.3: the "dead" catalog structs stay (2026-09-27)
- **Audit:** of the six structs the brief listed in `catalog.rs`:
  - `AgentInput` is **live**. `tool_registry.rs:212` parses every agent-as-tool call with it.
  - `FetchInput`, `FSListInput`, `FSFileInfoInput`, `UndoInput` and `SelectInput` are referenced nowhere else.
- **Decision: leave them.** They are never serialised to a model, so they cost no tokens and change no behaviour.
  They are also upstream code: deleting them gains nothing measurable and adds merge conflicts whenever upstream
  touches that part of the file (principle 8). If upstream removes them, the merge takes that for free.

## D-090 — Free-tier bake-off day 2: Dots ties the incumbent; the default stays (2026-09-27)
- **Run:** `dots-studio/dots-3-note-preview:free`, all 6 fixtures × 1 seed
  (`benchmarks/reports/models/20260926-free-suite-dots.md`), on a second, separate OpenRouter account supplied by
  the team (fresh 50/day; all 50 used). Conditions match the incumbent's round-1 record: the pre-D-088 binary, with
  the runtime gate and compact docs both off.
  - The per-fixture cap was **17**, the incumbent's own maximum (on `py-config-cli`). A first launch at 8 was
    stopped before its first request, because it would have failed Dots on a fixture the incumbent needed 17 calls
    for.
- **Result:** Dots **6/6**, 55 calls, 611 s. Per fixture, in calls:

  | Fixture | Calls |
  |---|---|
  | `integrity-trap` | 7 |
  | `js-duration` | 12 |
  | `node-feature` | 4 |
  | `py-bugfix` | 4 |
  | `py-config-cli` | 11 |
  | `py-ledger` | 17, at the cap |

- **Compared with the incumbent** (`nemotron-3-ultra`, round 1: 5/6, 40 calls): its one ✗, `py-ledger`, was a
  zero-call error, meaning the account, not the model. It passed `py-ledger` in all 4 NIM runs of the D-085 A/B.
  - On the five fixtures both completed: **Ultra 5/5 in 40 calls, Dots 5/5 in 38.** That is a tie within one
    seed's noise.
  - D-071's rule (higher success, or equal success with fewer calls) is not met in any way that one seed can
    carry.
- **Decision:** the default stays `nemotron-3-ultra`.
  - Dots is the strongest free challenger measured, and the first candidate for a fallback list (`FALLBACK_MODELS`,
    opt-in, D-081).
  - Two reasons against switching now: a "preview" model is an availability risk for a judged run, and replacing a
    default on a one-seed tie would be noise-driven.
  - A k = 3 comparison needs about 150 free requests, three account-days; it is worth it only if the default's
    availability becomes a problem.

## D-091 — `write_note` A/B on Kimi: the model never used the tool; the flag stays off (T3.14) (2026-09-27)
- **Run:** `2026-09-26-write-note-nim-kimi.md`, 6 fixtures × 1 seed. Both arms at `PEACH_COMPACT__MESSAGE_THRESHOLD=12`,
  so they compact (13 `context_compaction` events). The binary is frozen at `59be9d5`, so the gate is on in both
  arms.
- **Result:** 5/6 → 3/6, +17% input. **But the candidate made zero `write_note` calls.** The arms differed only by
  one extra tool definition. The swing is Kimi's own variance: in `py-ledger`, the candidate made no tool call at
  all and declared done, the D-085 behaviour the gate then caught twice.
- **Reading:** this says nothing about scratchpad notes, only that an unprompted Kimi does not pick up a new
  optional tool. The flag stays off, and the Nemotron 3.5 arm is still running. If no family uses the tool, the
  next step is a one-line mention in the agent prompt, a behaviour change measured with its own A/B, before
  judging the mechanism.

## D-092 — Local web UI for running tasks, watching them live, and browsing evidence (2026-09-27)
- **What it's for:** `make ui` starts a small Node HTTP server (`harness/ui/server.ts`) so a task can be
  started, watched, and its evidence browsed from a browser instead of memorizing CLI flags — useful for a live
  demo or for an evaluator who prefers it to the stdin/`PROMPT=` path. It does not replace or change `make run`'s
  one-shot contract: it shells out to the same `harness/run-task` entry point every `make run` uses, so there is
  no second execution path to keep in sync.
- **Safety, checked before shipping:** binds `127.0.0.1` only; every request must present a loopback `Host`
  header and every `POST` a matching `Origin` (defends DNS rebinding and CSRF); provider keys never reach the
  browser, only whether one is set; evidence/report file serving is path-checked against its root (`within()`);
  one run at a time (the free tiers this harness targets cannot afford parallel runs anyway); Stop sends SIGINT
  to the run's own process group, so it exits 5 with a complete evidence bundle exactly like a terminal Ctrl-C.
- **Proof:** `harness/ui/lib.test.ts`, 5/5 (key-to-provider matching, automatic profile selection, path
  containment, profile model ordering, telemetry line-splitting under partial writes). Smoke-tested live: started
  the server, hit `/api/status`, got a real profile list and the harness binary's path back.
- **Not yet run against a live evaluator or judge, and no A/B** — it is a convenience layer, not a behaviour
  change to the harness itself, so principle 6 does not apply the way it does to a flag inside the agent loop.

## D-093 — Tier 2 A/Bs, first family in for four flags: two closed, two pending, two confounded (2026-09-27)
- **T1.2 line numbers off — closed, stays off.** Two model families now (GLM: py-bugfix/node-feature/py-ledger,
  Ultra: all six fixtures, k=2). Success flat both times (100%→100% GLM, 92%→92% Ultra); cost worse both times
  (input +64.0%/+24.8%, calls +50.0%/+23.7%, wall +36.9%/+27.0%). No benefit on either family. `T1.2` ticked.
- **T2.1 parallel read-only and T2.6 tool correction — one family in, both look like regressions, second family
  still running.** GLM only so far (py-bugfix/node-feature/py-ledger, k=1): T2.1 success flat, cost worse
  (input +18.7%, calls +13.3%, wall +28.0% — notably calls and wall time both *rose* under a flag meant to run
  reads concurrently, the opposite of its point, worth a look at the implementation once Ultra's run is in, not
  just a "no benefit" verdict). T2.6 success flat, cost worse (input +33.1%, calls +26.7%, wall +30.1%). Left
  unticked pending the Ultra arm (MM.5); provisional reading is "no case for shipping either."
- **T3.10 handoff note (Muse Glimmer) — confounded, not usable as evidence either way.** Read alone, success
  rose 0%→33% — but every base run hit the request-limit ceiling (40-45 calls, all six fixtures), and two
  candidate runs didn't fail, they ran away: 158 calls / 6.19M input tokens on integrity-trap, 143 calls / 5.66M
  on js-duration, both hitting the 600s wall clock still going. The apparent success gain is base's request cap
  biting before candidate's, not the flag helping — and the runaway token counts are their own finding,
  independent of this flag: something about Muse Glimmer under these conditions does not converge. Not shipping
  on this; not concluding the flag is bad either. Needs a rerun with a cap sized for what Muse actually needs, or
  a different model, before this flag gets a real verdict.
- **T3.14 write_note (Nemotron 3.5) — also confounded, also not concluding.** Success fell 50%→33%, but four of
  six fixtures interrupted on *both* arms (hit the request or duration cap before finishing), so the samples
  that actually completed are too few to read as the flag's effect rather than which runs happened to fit the
  cap. BR.3's own note was right: this needs runs long enough to actually compact, which these were not.
- **Updated `TASKS.md`:** T1.2 ticked closed. T2.1/T2.6 left unticked with the GLM-only reading recorded.
  T3.10/T3.14's notes now carry their A/B results and the confound in each, rather than staying silent pending.

## D-094 — T1.5 noise compression on GLM: no benefit, stays off (2026-09-27)
- **Run:** `2026-09-27-noise-compression-nim-glm.md`, py-bugfix/node-feature/py-ledger, k=1. Success flat
  100%→100%. Cost worse across the board: input +9.4%, output +7.5%, calls +7.1%, wall +21.1%.
- **Reading:** same pattern as T1.2/T2.1/T2.6's GLM arms — no case for shipping. Second family not yet run;
  leaving unticked rather than closing outright, consistent with T2.1/T2.6's treatment in D-093.
