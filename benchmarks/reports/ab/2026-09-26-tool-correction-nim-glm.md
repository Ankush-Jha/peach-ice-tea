# A/B — tool-correction-nim-glm

Profile `nvidia-nim` (development) · suite `py-bugfix,node-feature,py-ledger` · seeds 1 · 2026-09-26T23:07:45.746Z

Base: `{"PEACH_HARNESS_COMPACT_TOOL_DOCS":"1","PEACH_HARNESS_TOOL_CORRECTION":"0"}` · Candidate: `{"PEACH_HARNESS_COMPACT_TOOL_DOCS":"1","PEACH_HARNESS_TOOL_CORRECTION":"1"}`

| Metric | Base | Candidate | Δ |
|---|---|---|---|
| Success (95% CI) | 100% [44–100%] | 100% [44–100%] | 0 pts |
| Integrity violations | 0 | 0 | |
| Input tokens / run | 62,843 | 83,620 | +33.1% |
| Output tokens / run | 1,670 | 1,533 | -8.2% |
| LLM calls / run | 5.0 | 6.3 | +26.7% |
| Wall time / run | 418 s | 544 s | +30.1% |

## Per run

| Fixture | Seed | Arm | Success | Outcome | Calls | Input tok | Output tok | Wall s |
|---|---|---|---|---|---|---|---|---|
| node-feature | 1 | base | yes | completed | 4 | 43138 | 496 | 333 |
| node-feature | 1 | cand | yes | completed | 4 | 42481 | 564 | 379 |
| py-bugfix | 1 | base | yes | completed | 4 | 42775 | 450 | 249 |
| py-bugfix | 1 | cand | yes | completed | 4 | 42741 | 527 | 268 |
| py-ledger | 1 | base | yes | completed | 7 | 102615 | 4064 | 672 |
| py-ledger | 1 | cand | yes | completed | 11 | 165638 | 3509 | 986 |

Small samples: read the success CI before any token delta. A token saving that costs success is a regression (principle 1).
