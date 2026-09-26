# A/B — line-numbers-off-nim-glm

Profile `nvidia-nim` (development) · suite `py-bugfix,node-feature,py-ledger` · seeds 1 · 2026-09-26T22:15:01.356Z

Base: `{"FORGE_HARNESS_COMPACT_TOOL_DOCS":"1","FORGE_HARNESS_LINE_NUMBERS_OFF":"0"}` · Candidate: `{"FORGE_HARNESS_COMPACT_TOOL_DOCS":"1","FORGE_HARNESS_LINE_NUMBERS_OFF":"1"}`

| Metric | Base | Candidate | Δ |
|---|---|---|---|
| Success (95% CI) | 100% [44–100%] | 100% [44–100%] | 0 pts |
| Integrity violations | 0 | 0 | |
| Input tokens / run | 55,906 | 91,714 | +64.0% |
| Output tokens / run | 1,330 | 3,051 | +129.4% |
| LLM calls / run | 4.7 | 7.0 | +50.0% |
| Wall time / run | 423 s | 580 s | +36.9% |

## Per run

| Fixture | Seed | Arm | Success | Outcome | Calls | Input tok | Output tok | Wall s |
|---|---|---|---|---|---|---|---|---|
| node-feature | 1 | base | yes | completed | 4 | 43079 | 682 | 380 |
| node-feature | 1 | cand | yes | completed | 4 | 42768 | 469 | 305 |
| py-bugfix | 1 | base | yes | completed | 4 | 42745 | 440 | 375 |
| py-bugfix | 1 | cand | yes | completed | 4 | 42614 | 540 | 314 |
| py-ledger | 1 | base | yes | completed | 6 | 81895 | 2869 | 515 |
| py-ledger | 1 | cand | yes | completed | 13 | 189759 | 8145 | 1121 |

Small samples: read the success CI before any token delta. A token saving that costs success is a regression (principle 1).
