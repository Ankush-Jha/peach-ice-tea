# Local development loop

Verified on macOS (darwin 27.0.0, arm64) against upstream commit `304bf3b` on 2026-09-20.

## Prerequisites

| Tool | Version verified | Notes |
|---|---|---|
| Rust toolchain | 1.98.1 | Pinned by `rust-toolchain.toml` (`channel = "1.98"`). `rustup` installs it automatically on first `cargo` invocation in this directory. |
| `protoc` | 36.2 | **Undocumented upstream prerequisite — the build fails without it.** See below. |
| `cargo-insta` | 1.48.0 | Must match the `insta` version in `Cargo.lock` (currently 1.48.0). |
| `cargo-nextest` | 0.9.145 | Required: `insta.yaml` sets `runner: nextest`. `cargo insta test` fails with `no such command: nextest` otherwise. |
| Node | 20+ (tested on 25.5.0) | For `benchmarks/` (TypeScript, run via `tsx`). |
| Docker | any recent (tested 29.4.3) | For the TermBench/Harbor suite (T0.5). |

### `protoc` is required

`crates/forge_repo` has a `build.rs` that runs `tonic_prost_build::compile_protos("proto/forge.proto")`.
Without `protoc` on `PATH`, a clean clone fails at:

```
error: failed to run custom build command for `forge_repo v0.1.0`
  Could not find `protoc`. ... To install it on macOS, run `brew install protobuf`
```

Install it before anything else:

```bash
brew install protobuf          # macOS
# apt-get install -y protobuf-compiler   # Debian/Ubuntu
```

Note that `forge_repo` is **not** a diesel-only crate, contrary to the impression given by
RESEARCH.md's persistence section: it also carries `tonic`/`prost` and a real
`crates/forge_repo/proto/forge.proto`. The gRPC client (`forge_infra/src/grpc.rs`) talks to an
external workspace server for semantic search; it is unrelated to agent chat.

## One-time setup

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.98 --profile default
brew install protobuf
cargo install cargo-insta --locked
cargo install cargo-nextest --locked
npm install                     # for benchmarks/
```

## Verification loop

Per `CLAUDE.md`, verify with `cargo check` then `cargo insta test`. **Never** `cargo build --release`
for verification.

```bash
cargo check --workspace --all-targets
cargo insta test --workspace
```

Clean-clone baseline at `304bf3b` (recorded for T0.1):

- `cargo check --workspace --all-targets` — clean, exit 0. One future-incompat warning from the
  third-party `proc-macro-error2 v2.0.1`; not ours, not actionable.
- `cargo insta test --workspace` — **2678 tests run, 2678 passed, 1 skipped, 0 failed.**
  No pending snapshots (`info: no snapshots to review`).
- Cold `target/` after both: ~1.7 GB. Budget several GB of disk.

Because the suite is fully green upstream, `CLAUDE.md`'s rule about pre-existing upstream test
failures does not currently apply — any red test is ours.

## Running evals

```bash
npm run eval ./benchmarks/evals/<name>/task.yml
LOG_LEVEL=debug npm run eval ./benchmarks/evals/<name>/task.yml
```

The evals shell out to a binary named `forgee`, which is a **manually created** symlink to
`target/debug/forge` (tasks run in ephemeral temp dirs, so relative paths don't work):

```bash
ln -sf $(pwd)/target/debug/forge ~/bin/forgee   # ~/bin must be on PATH
```

## Provider credentials

Set the provider's key (`OPENROUTER_API_KEY` for the eval arms — see
`crates/forge_repo/src/provider/provider.json` for the full provider-to-variable mapping). Forge prints
"Forge no longer reads API keys from environment variables" and then migrates the key **once** into its
stored credentials; the variable is not re-read per request. Keep the key in a mode-600 file sourced from
your shell profile rather than in `~/.zshrc` itself, which is world-readable.

> **Warning — the eval harness does not currently work.** 10 of the 14 evals invoke
> `forgee --provider ... --model ...`, flags removed from the CLI in `b3ec4d17a` (clap exits 2), and
> `todo_write_usage` uses `FORGE_OVERRIDE_PROVIDER`/`FORGE_OVERRIDE_MODEL`, which map to no config field.
> See `docs/harness/RECON.md` §2 and task **T0.0**. Do not interpret a 0% pass rate as a regression
> until T0.0 is done.

Any `ForgeConfig` field can be set per-process via a `FORGE_<FIELD>` env var (`__` separates nested
fields; a single `_` does not). This is parallel-safe — unlike `forge config set`, which does an
unlocked read-modify-write of a shared `~/.forge/.forge.toml`. Useful ones:

```bash
FORGE_SESSION__PROVIDER_ID=open_router FORGE_SESSION__MODEL_ID=anthropic/claude-sonnet-4.6   # select provider+model
FORGE_DEBUG_REQUESTS=/path/to/context.json    # append each outgoing provider request body (JSONL)
FORGE_AUTO_DUMP=json                          # write a full structured Conversation dump on TaskComplete
```

## Guardrails

- `origin` and `upstream` both point at `https://github.com/tailcallhq/forgecode.git` (plain clone,
  no fork). **Never push to either.** Create a fork and repoint `origin` before any push.
- API keys come from the environment only (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`). Never write them
  to files, logs or reports.

## Running the hackathon suite on a provider profile

`configuration/profiles/<name>/` holds a `forge.toml` (provider, model) and a `profile.env` (non-secret
settings plus `PROFILE_KEY_VAR`, the key variable the profile needs). The runner materialises it into an
isolated `FORGE_CONFIG`, so `~/.forge` is never read or written, and it strips every other provider's key
from forge's environment.

```bash
# Evaluation profile (the only one allowed in a judged run):
GEMINI_API_KEY=... npm run hackathon -- --agent forge --profile gemini --suite py-bugfix \
  --max-duration-secs 600 --max-requests 40
# Development profile (not for judged runs, D-036):
DEEPSEEK_API_KEY=... npm run hackathon -- --agent forge --profile deepseek --suite py-bugfix
```

Estimate the cost before any live run (D-025). The `₹100` budget is for Gemini; DeepSeek spend needs its own
agreement.
