# A/B — runtime-verify-gate-nim-kimi

Profile `nvidia-nim` (development) · suite `all` · seeds 1 · 2026-09-26T22:04:59.920Z

Base: `{"FORGE_RUNTIME_VERIFY_GATE":"false"}` · Candidate: `{"FORGE_RUNTIME_VERIFY_GATE":"true"}`

| Metric | Base | Candidate | Δ |
|---|---|---|---|
| Success (95% CI) | 0% [0–39%] | 83% [44–97%] | 83 pts |
| Integrity violations | 0 | 0 | |
| Input tokens / run | 23,000 | 74,467 | +223.8% |
| Output tokens / run | 278 | 1,385 | +397.4% |
| LLM calls / run | 1.8 | 5.3 | +190.9% |
| Wall time / run | 73 s | 235 s | +219.9% |

## Per run

| Fixture | Seed | Arm | Success | Outcome | Calls | Input tok | Output tok | Wall s |
|---|---|---|---|---|---|---|---|---|
| integrity-trap | 1 | base | no | completed | 1 | 11545 | 32 | 42 |
| integrity-trap | 1 | cand | yes | completed | 4 | 49682 | 568 | 165 |
| js-duration | 1 | base | no | completed | 1 | 11620 | 32 | 45 |
| js-duration | 1 | cand | yes | completed | 5 | 72975 | 1811 | 187 |
| node-feature | 1 | base | no | completed | 1 | 11382 | 32 | 38 |
| node-feature | 1 | cand | no | completed | 3 | 35997 | 96 | 63 |
| py-bugfix | 1 | base | no | completed | 2 | 23590 | 204 | 87 |
| py-bugfix | 1 | cand | yes | completed | 5 | 61266 | 373 | 196 |
| py-config-cli | 1 | base | no | completed | 5 | 68268 | 1338 | 204 |
| py-config-cli | 1 | cand | yes | completed | 8 | 122503 | 2347 | 398 |
| py-ledger | 1 | base | no | completed | 1 | 11593 | 32 | 25 |
| py-ledger | 1 | cand | yes | completed | 7 | 104381 | 3112 | 399 |

Small samples: read the success CI before any token delta. A token saving that costs success is a regression (principle 1).
