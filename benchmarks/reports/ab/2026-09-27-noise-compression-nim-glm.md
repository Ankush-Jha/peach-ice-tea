# A/B — noise-compression-nim-glm

Profile `nvidia-nim` (development) · suite `py-bugfix,node-feature,py-ledger` · seeds 1 · 2026-09-27T00:00:39.628Z

Base: `{"PEACH_HARNESS_COMPACT_TOOL_DOCS":"1","PEACH_HARNESS_NOISE_COMPRESSION":"0"}` · Candidate: `{"PEACH_HARNESS_COMPACT_TOOL_DOCS":"1","PEACH_HARNESS_NOISE_COMPRESSION":"1"}`

| Metric | Base | Candidate | Δ |
|---|---|---|---|
| Success (95% CI) | 100% [44–100%] | 100% [44–100%] | 0 pts |
| Integrity violations | 0 | 0 | |
| Input tokens / run | 54,075 | 59,137 | +9.4% |
| Output tokens / run | 1,255 | 1,349 | +7.5% |
| LLM calls / run | 4.7 | 5.0 | +7.1% |
| Wall time / run | 494 s | 598 s | +21.1% |

## Per run

| Fixture | Seed | Arm | Success | Outcome | Calls | Input tok | Output tok | Wall s |
|---|---|---|---|---|---|---|---|---|
| node-feature | 1 | base | yes | completed | 4 | 42491 | 393 | 531 |
| node-feature | 1 | cand | yes | completed | 4 | 42585 | 587 | 652 |
| py-bugfix | 1 | base | yes | completed | 4 | 43061 | 554 | 446 |
| py-bugfix | 1 | cand | yes | completed | 4 | 42707 | 426 | 490 |
| py-ledger | 1 | base | yes | completed | 6 | 76672 | 2819 | 506 |
| py-ledger | 1 | cand | yes | completed | 7 | 92120 | 3034 | 653 |

Small samples: read the success CI before any token delta. A token saving that costs success is a regression (principle 1).
