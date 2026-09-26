# A/B — parallel-readonly-nim-glm

Profile `nvidia-nim` (development) · suite `py-bugfix,node-feature,py-ledger` · seeds 1 · 2026-09-26T22:40:31.558Z

Base: `{"PEACH_HARNESS_COMPACT_TOOL_DOCS":"1","PEACH_HARNESS_PARALLEL_READONLY":"0"}` · Candidate: `{"PEACH_HARNESS_COMPACT_TOOL_DOCS":"1","PEACH_HARNESS_PARALLEL_READONLY":"1"}`

| Metric | Base | Candidate | Δ |
|---|---|---|---|
| Success (95% CI) | 100% [44–100%] | 100% [44–100%] | 0 pts |
| Integrity violations | 0 | 0 | |
| Input tokens / run | 58,150 | 69,035 | +18.7% |
| Output tokens / run | 1,273 | 1,721 | +35.3% |
| LLM calls / run | 5.0 | 5.7 | +13.3% |
| Wall time / run | 398 s | 509 s | +28.0% |

## Per run

| Fixture | Seed | Arm | Success | Outcome | Calls | Input tok | Output tok | Wall s |
|---|---|---|---|---|---|---|---|---|
| node-feature | 1 | base | yes | completed | 6 | 66272 | 817 | 324 |
| node-feature | 1 | cand | yes | completed | 4 | 42700 | 616 | 313 |
| py-bugfix | 1 | base | yes | completed | 4 | 42765 | 426 | 275 |
| py-bugfix | 1 | cand | yes | completed | 4 | 42825 | 455 | 318 |
| py-ledger | 1 | base | yes | completed | 5 | 65413 | 2575 | 594 |
| py-ledger | 1 | cand | yes | completed | 9 | 121579 | 4093 | 896 |

Small samples: read the success CI before any token delta. A token saving that costs success is a regression (principle 1).
