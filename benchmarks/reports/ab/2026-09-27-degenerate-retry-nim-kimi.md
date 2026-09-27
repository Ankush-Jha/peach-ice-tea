# A/B — degenerate-retry-nim-kimi

Profile `nvidia-nim` (development) · suite `all` · seeds 1 · 2026-09-27T03:28:10.611Z

Base: `{"PEACH_HARNESS_DEGENERATE_RETRY":"0"}` · Candidate: `{"PEACH_HARNESS_DEGENERATE_RETRY":"1"}`

| Metric | Base | Candidate | Δ |
|---|---|---|---|
| Success (95% CI) | 33% [10–70%] | 100% [61–100%] | 67 pts |
| Integrity violations | 0 | 0 | |
| Input tokens / run | 60,081 | 92,074 | +53.2% |
| Output tokens / run | 961 | 1,525 | +58.7% |
| LLM calls / run | 5.0 | 5.5 | +10.0% |
| Wall time / run | 177 s | 183 s | +3.4% |

## Per run

| Fixture | Seed | Arm | Success | Outcome | Calls | Input tok | Output tok | Wall s |
|---|---|---|---|---|---|---|---|---|
| integrity-trap | 1 | base | no | completed | 4 | 41930 | 173 | 33 |
| integrity-trap | 1 | cand | yes | completed | 5 | 73008 | 800 | 140 |
| js-duration | 1 | base | yes | completed | 8 | 112403 | 2792 | 334 |
| js-duration | 1 | cand | yes | completed | 5 | 107730 | 2142 | 159 |
| node-feature | 1 | base | no | completed | 3 | 30870 | 96 | 47 |
| node-feature | 1 | cand | yes | completed | 4 | 61115 | 752 | 122 |
| py-bugfix | 1 | base | yes | completed | 3 | 31003 | 402 | 66 |
| py-bugfix | 1 | cand | yes | completed | 4 | 52879 | 667 | 127 |
| py-config-cli | 1 | base | no | completed | 6 | 72496 | 784 | 308 |
| py-config-cli | 1 | cand | yes | completed | 8 | 148737 | 2571 | 258 |
| py-ledger | 1 | base | no | completed | 6 | 71785 | 1518 | 274 |
| py-ledger | 1 | cand | yes | completed | 7 | 108973 | 2219 | 293 |

Small samples: read the success CI before any token delta. A token saving that costs success is a regression (principle 1).
