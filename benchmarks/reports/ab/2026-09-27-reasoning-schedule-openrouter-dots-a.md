# A/B — reasoning-schedule-openrouter-dots-a

Profile `openrouter` (evaluation) · suite `py-bugfix,node-feature` · seeds 1 · 2026-09-27T04:12:13.631Z

Base: `{"PEACH_HARNESS_REASONING_SCHEDULE":"0"}` · Candidate: `{"PEACH_HARNESS_REASONING_SCHEDULE":"1"}`

| Metric | Base | Candidate | Δ |
|---|---|---|---|
| Success (95% CI) | 100% [34–100%] | 100% [34–100%] | 0 pts |
| Integrity violations | 0 | 0 | |
| Input tokens / run | 70,184 | 55,937 | -20.3% |
| Output tokens / run | 1,120 | 854 | -23.8% |
| LLM calls / run | 6.0 | 5.0 | -16.7% |
| Wall time / run | 24 s | 19 s | -20.9% |

## Per run

| Fixture | Seed | Arm | Success | Outcome | Calls | Input tok | Output tok | Wall s |
|---|---|---|---|---|---|---|---|---|
| node-feature | 1 | base | yes | completed | 7 | 84421 | 1279 | 28 |
| node-feature | 1 | cand | yes | completed | 5 | 55538 | 847 | 18 |
| py-bugfix | 1 | base | yes | completed | 5 | 55947 | 961 | 20 |
| py-bugfix | 1 | cand | yes | completed | 5 | 56336 | 861 | 20 |

Small samples: read the success CI before any token delta. A token saving that costs success is a regression (principle 1).
