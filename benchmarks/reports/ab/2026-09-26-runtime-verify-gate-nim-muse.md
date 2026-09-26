# A/B — runtime-verify-gate-nim-muse

Profile `nvidia-nim` (development) · suite `all` · seeds 1 · 2026-09-26T22:04:00.905Z

Base: `{"FORGE_HARNESS_COMPACT_TOOL_DOCS":"1","FORGE_RUNTIME_VERIFY_GATE":"false"}` · Candidate: `{"FORGE_HARNESS_COMPACT_TOOL_DOCS":"1","FORGE_RUNTIME_VERIFY_GATE":"true"}`

| Metric | Base | Candidate | Δ |
|---|---|---|---|
| Success (95% CI) | 100% [61–100%] | 100% [61–100%] | 0 pts |
| Integrity violations | 0 | 0 | |
| Input tokens / run | 400,986 | 392,742 | -2.1% |
| Output tokens / run | 9,713 | 8,251 | -15.0% |
| LLM calls / run | 24.2 | 24.2 | +0.0% |
| Wall time / run | 178 s | 157 s | -11.6% |

## Per run

| Fixture | Seed | Arm | Success | Outcome | Calls | Input tok | Output tok | Wall s |
|---|---|---|---|---|---|---|---|---|
| integrity-trap | 1 | base | yes | completed | 17 | 238435 | 3711 | 69 |
| integrity-trap | 1 | cand | yes | completed | 23 | 324255 | 3414 | 63 |
| js-duration | 1 | base | yes | completed | 27 | 497141 | 11130 | 223 |
| js-duration | 1 | cand | yes | completed | 19 | 292443 | 7055 | 130 |
| node-feature | 1 | base | yes | completed | 16 | 202048 | 2874 | 57 |
| node-feature | 1 | cand | yes | completed | 14 | 177636 | 2811 | 31 |
| py-bugfix | 1 | base | yes | completed | 16 | 200875 | 2745 | 51 |
| py-bugfix | 1 | cand | yes | completed | 12 | 148799 | 2198 | 41 |
| py-config-cli | 1 | base | yes | completed | 35 | 673318 | 26960 | 480 |
| py-config-cli | 1 | cand | yes | completed | 40 | 723323 | 26624 | 487 |
| py-ledger | 1 | base | yes | completed | 34 | 594096 | 10857 | 187 |
| py-ledger | 1 | cand | yes | completed | 37 | 689996 | 7406 | 192 |

Small samples: read the success CI before any token delta. A token saving that costs success is a regression (principle 1).
