#!/usr/bin/env bash
# Builds the forge binary that harness/peach-ice-tea runs (release, for distribution).
# Prerequisites: Rust (rust-toolchain.toml pins the version) and `protoc` (D-009).
set -euo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
command -v protoc >/dev/null || { echo "build.sh: protoc is required (see docs/harness/DEV.md)" >&2; exit 1; }
cargo build --release --manifest-path "$REPO/Cargo.toml" -p forge_main
echo "built: $REPO/target/release/forge"
