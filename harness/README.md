# harness/

| File | Purpose |
|---|---|
| `peach-ice-tea` | The evaluation entry point: one frozen, unattended run with the `gemini` profile, writing the evidence bundle. `--help` for usage. |
| `build.sh` | Release build of the `forge` binary the entry point runs. |

The harness itself is the Rust workspace in `crates/` (a ForgeCode fork; see the root README for the
map). It stays there rather than under `harness/` so upstream merges remain possible (D-004, D-021).
