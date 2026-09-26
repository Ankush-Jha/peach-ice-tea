# CLAUDE.md — harness project

You are working in a **fork of ForgeCode** (`tailcallhq/forgecode`, Rust). We are turning it
into a best-in-class coding-agent harness. This file loads every session, so it stays short;
the detail lives in `docs/harness/`.

## Who you're working for
Ankush — Head of AI Product and Engineering at Superkreatives; publishes research on AI
orchestration. How he wants you to work:
- **Make decisions when uncertain.** Pick the option the evidence favours, record it in
  `docs/harness/DECISIONS.md`, and keep going. Only stop and ask for destructive or
  irreversible actions, scope changes, or spend above the eval budget below.
- **Research-grounded.** Every non-trivial change cites a requirement ID from
  `docs/harness/SPEC.md`, which traces to evidence in `docs/harness/RESEARCH.md`.
- **Visual summaries.** End-of-task reports use a short table or a Mermaid diagram rather
  than long prose.

## Read before working
1. `docs/harness/TASKS.md` — find the first unticked task whose dependencies are done.
2. The requirement(s) it cites in `docs/harness/SPEC.md`.
3. The linked evidence in `docs/harness/RESEARCH.md` if the requirement is unclear.
4. `docs/harness/DECISIONS.md` for choices already made.

## Non-negotiable principles
1. **Optimise the completed task, not the single tool call.** Fewer tokens per call that
   cause extra turns is a regression.
2. **Lossless → reversible → lossy.** Never remove information without a recovery path
   unless the spec's last-resort stage says so.
3. **Enforce in the runtime, don't rely on prompts** for behaviour that matters.
4. **Make withheld information loud** in plain text inside the model's context.
5. **Fail open.** Optional subsystems fall back to current behaviour on error or timeout.
6. **No efficiency/behaviour change ships without an A/B report** (`benchmarks/reports/`)
   on both model families. If the report doesn't support shipping, ship behind a flag
   (default off) or revert — and say so.
7. **Don't regress Forge's strengths:** schema rules (`required` before `properties`, flat
   schemas), enforced verification, todo enforcement, doom-loop detection, reasoning-chain
   preservation in compaction.
8. **Stay mergeable with upstream:** additive modules, minimal edits to upstream files,
   keep crate names.

## Code conventions
@AGENTS.md

Override: ignore the `Co-Authored-By: ForgeCode` commit-trailer rule in `AGENTS.md`; use
normal Claude Code attribution.

## Working loop for each task
1. Branch `harness/<task-id>-<slug>`.
2. Write a short plan in the PR description: requirement IDs, files to touch, tests, how
   success will be measured.
3. Tests first where practical (in-file unit tests per `AGENTS.md`; `insta` snapshots for
   tool output).
4. Implement. Verify with `cargo check`, then `cargo insta test --accept` for affected crates.
   **Never** `cargo build --release` for verification.
5. For **[A/B]** tasks: run `benchmarks` A/B vs the last shipped state; commit the report.
6. Tick the task in `TASKS.md` with the report link; update `DECISIONS.md` if you chose
   anything; update `forge.schema.json` and `crates/forge_config/.forge.toml` for new config.
7. Finish with a summary table: task, requirement IDs, files changed, test result, A/B
   headline (success Δ, input tokens Δ, LLM calls Δ, wall time Δ, recovery rate), decision.

## Guardrails
- Never push to the `upstream` remote; never force-push `main`.
- API keys only from environment variables; never write them to files, logs or reports.
- Redact secrets (see `R-SAFE-3`) before any transcript leaves the process.
- Eval spend: token cost is **not** the project's constraint (D-050); choose models on merit and run
  real A/Bs (k = 3). The one hard ceiling is the OpenRouter key's own $50 limit: record actual
  tokens from the response and keep a running total in `DECISIONS.md`.
- Don't call ForgeCode Services APIs or depend on the proprietary Jev scorer.
- If an upstream test fails before your change, record it in `DECISIONS.md` and don't
  "fix" it by weakening the test.

## Useful map (paths under `crates/`)
- Loop: `forge_app/src/orch.rs` · hooks wiring: `forge_app/src/app.rs` · hooks: `forge_app/src/hooks/`
- Tool output rendering: `forge_app/src/operation.rs` · truncation: `forge_app/src/truncation/`
- Tool definitions/descriptions: `forge_domain/src/tools/catalog.rs`, `forge_domain/src/tools/descriptions/`
- Compaction today: `forge_app/src/compact.rs`, `forge_domain/src/compact/`, `templates/forge-partial-summary-frame.md`
- Persistence: `forge_repo/src/database/` (diesel), `forge_repo/src/conversation/`
- Permissions: `forge_services/src/permissions.default.yaml`, `forge_services/src/policy.rs`
- Config defaults: `forge_config/.forge.toml` · evals: `../benchmarks/`
