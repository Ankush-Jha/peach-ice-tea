# Decision log

Append-only. Format: ID, date, decision, context, alternatives, consequences. Claude Code:
when the spec is silent or ambiguous, **make the call, record it here, and continue** —
don't stop to ask unless the choice is destructive, irreversible, or changes scope.

## D-001 — Fork ForgeCode rather than wrap it (2026-09-20)
- **Context:** compaction (`forge_app/src/compact.rs`), tool-output rendering
  (`operation.rs`) and the loop (`orch.rs`) are internal; Forge exposes no external hooks.
- **Alternatives:** wrapper process around the CLI; plugin via MCP.
- **Consequences:** we can change internals directly; we carry a merge burden → D-004.

## D-002 — Append-only event log; context becomes a projection (2026-09-20)
- **Context:** Forge stores one `context` blob per conversation and compaction overwrites it,
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

## D-005 — Rebuild ForgeCode Services capabilities in the open, selectively (2026-09-20)
- **Context:** Forge's Part 1 credits five capabilities to a proprietary Services layer
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
  `https://github.com/tailcallhq/forgecode.git`. No fork created on the user's GitHub account.
- **Consequences:** nothing can be pushed until a fork exists and `origin` is repointed. Recorded as a
  guardrail in `docs/harness/DEV.md`. Clean-clone baseline verified: `cargo check --workspace
  --all-targets` clean; `cargo insta test --workspace` = 2678 passed, 1 skipped, 0 failed, no pending
  snapshots. `CLAUDE.md`'s pre-existing-upstream-failure rule therefore has nothing to record.

## D-009 — `protoc` is a build prerequisite; document it rather than vendor it (2026-09-20)
- **Context:** a clean clone fails at `forge_repo`'s build script (`tonic_prost_build::compile_protos`)
  with "Could not find `protoc`". `START_HERE.md` lists only Rust, cargo-insta, Node and Docker.
- **Alternatives:** vendor a pinned `protoc` via the `protobuf-src` crate (removes the system dependency
  but adds build time and a patch to an upstream `Cargo.toml`, against D-004).
- **Decision:** document it in `docs/harness/DEV.md` and in any CI job we add; do not patch upstream files.
- **Consequences:** one extra setup step; no merge burden. Verified with protobuf 36.2.

## D-010 — R-LOOP-4's premise is wrong: non-interactive mode already exists (2026-09-20)
- **Context:** RESEARCH.md S1c claims "No non-interactive profile found in open source." It does exist:
  `forge -p "<prompt>"` and piped stdin, gated by `Cli::is_interactive()`
  (`crates/forge_main/src/cli.rs:21,75`) and dispatched at `crates/forge_main/src/ui.rs:377`. It runs one
  turn through the same `Orchestrator::run` loop and exits, so doom-loop detection, compaction and
  `max_requests_per_turn` already apply there.
- **Decision:** re-scope T0.3 and T2.4. T0.3 adds a machine-readable output layer and outcome-aware exit
  codes to the **existing** dispatch path rather than building `forge exec` from nothing; the two failure
  reasons it must distinguish (`MaxToolFailurePerTurnLimitReached`, `MaxRequestPerTurnLimitReached`)
  already ride on `ChatResponse::Interrupt`. T2.4's remaining work is the prompt variant only.
- **Consequences:** less work than the spec assumed, and a smaller diff against upstream. Two facts to
  respect: `-p` currently always exits 0 regardless of outcome, and `--porcelain` is wired only to
  metadata subcommands, never to the chat path. Evidence in `docs/harness/RECON.md` §3.

## D-011 — Repair the eval harness before any baseline (new task T0.0) (2026-09-20)
- **Context:** 10 of the 14 evals invoke `forgee --provider ... --model ...`; those flags were removed
  upstream in `b3ec4d17a` (#2685). 11 rely on `FORGE_DEBUG_REQUESTS`, which has never existed in
  `crates/`. `benchmarks/` was last touched 2026-04-10, after the removal, without being updated.
  T0.4, T0.5 and T0.6 all depend on a harness that cannot currently start the agent.
- **Alternatives:** fold the repair into T0.3 (fewer tasks, but couples a Rust change to a TypeScript
  repair and delays discovering further harness rot); or baseline only the 4 working evals (a baseline
  too narrow to gate 15 later A/Bs).
- **Decision:** add **T0.0** ahead of T0.4 — repair the invocations, standardize model naming across
  `task.yml` files, and choose a replacement for the `FORGE_DEBUG_REQUESTS` transcript-scraping
  convention. Prefer `FORGE_SESSION__PROVIDER_ID`/`FORGE_SESSION__MODEL_ID`
  (`crates/forge_config/src/reader.rs:276`) over a `forge config set` prelude step, since env vars keep
  each eval row independent and parallel-safe.
- **Consequences:** M0 grows one task. A failed baseline can no longer be misread as a regression.
  The longer-term fix is that T0.3's JSON metrics line should replace `jq`-scraping outright, so T0.0
  should avoid investing in the transcript-dump approach beyond what unblocks a baseline.

## D-012 — Correction to D-011: `FORGE_DEBUG_REQUESTS` works; T0.0 is TypeScript-only (2026-09-20)
- **Context:** D-011 and the first draft of `RECON.md` §2 stated that `FORGE_DEBUG_REQUESTS` "has never existed in
  `crates/`", inferred from `grep -rn FORGE_DEBUG_REQUESTS crates/` returning zero hits. The grep is accurate; the
  inference was wrong. `ForgeConfig` has a `debug_requests: Option<PathBuf>` field
  (`forge_config/src/config.rs:173`) and `ConfigReader::read_env()` (`forge_config/src/reader.rs:104`) maps every
  `FORGE_<FIELD>` env var onto `ForgeConfig` generically, with `__` as the only nesting separator — so no literal
  string exists to grep for. It is consumed by `write_debug_request` (`forge_infra/src/http.rs:238`).
- **Verified empirically** against `target/debug/forge`: `FORGE_DEBUG_REQUESTS=/tmp/probe.json forge config list`
  prints `debug_requests = "/tmp/probe.json"`; `FORGE_SESSION__PROVIDER_ID`/`FORGE_SESSION__MODEL_ID` populate
  `[session]`; `FORGE_AUTO_DUMP=json` sets `auto_dump`. Conversely `--provider` exits 2
  ("unexpected argument"), and `FORGE_OVERRIDE_PROVIDER`/`FORGE_OVERRIDE_MODEL` — used only by `todo_write_usage`,
  a third naming scheme D-011 missed — map to no `ForgeConfig` field and are silently ignored.
- **Decision:** T0.0 stays in scope but shrinks to a TypeScript-only repair of `benchmarks/evals/*/task.yml`:
  replace `--provider X --model Y` and the `FORGE_OVERRIDE_*` pair with
  `FORGE_SESSION__PROVIDER_ID=X FORGE_SESSION__MODEL_ID=Y`, and **keep** `FORGE_DEBUG_REQUESTS` as-is. No Rust
  change, and no need to choose a replacement for the transcript-dump convention as D-011 assumed.
- **Consequences:** M0 is less blocked than D-011 implied — the only genuinely dead mechanisms are the two
  provider/model ones. Two caveats carry forward to T0.4 and are recorded in `RECON.md` §2: `debug_requests` is
  JSONL and captures requests only (never the final assistant message), and the evals' `jq` filters assume the
  OpenAI wire shape, so an Anthropic-native arm would silently match nothing. R-EVAL-1 requires ≥2 model families,
  so the Anthropic arm must either route through an OpenAI-compatible gateway or switch to
  `FORGE_AUTO_DUMP=json`, whose `Context` structure is provider-agnostic and also carries tool results and the
  final message.
- **Process note:** the error was reasoning from the absence of a grep hit to the absence of a feature. For a
  config value, check the config struct and its env-mapping layer before concluding it is unimplemented.
