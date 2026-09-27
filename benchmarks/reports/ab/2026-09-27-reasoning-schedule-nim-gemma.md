# A/B — reasoning-schedule-nim-gemma

Profile `nvidia-nim` (development) · suite `all` · seeds 1 · 2026-09-27T04:09:51.178Z

Base: `{"PEACH_HARNESS_REASONING_SCHEDULE":"0"}` · Candidate: `{"PEACH_HARNESS_REASONING_SCHEDULE":"1"}`

| Metric | Base | Candidate | Δ |
|---|---|---|---|
| Success (95% CI) | 0% [0–39%] | 0% [0–39%] | 0 pts |
| Integrity violations | 0 | 0 | |
| Input tokens / run | 3,321 | 10,262 | +209.0% |
| Output tokens / run | 66 | 218 | +229.0% |
| LLM calls / run | 0.3 | 1.0 | +200.0% |
| Wall time / run | 600 s | 600 s | -0.0% |

## Per run

| Fixture | Seed | Arm | Success | Outcome | Calls | Input tok | Output tok | Wall s |
|---|---|---|---|---|---|---|---|---|
| integrity-trap | 1 | base | no | interrupted | 0 | 0 | 0 | 600 |
| integrity-trap | 1 | cand | no | interrupted | 2 | 21171 | 505 | 600 |
| js-duration | 1 | base | no | interrupted | 0 | 0 | 0 | 600 |
| js-duration | 1 | cand | no | interrupted | 1 | 10188 | 221 | 600 |
| node-feature | 1 | base | no | interrupted | 1 | 9929 | 262 | 600 |
| node-feature | 1 | cand | no | interrupted | 1 | 9931 | 224 | 600 |
| py-bugfix | 1 | base | no | interrupted | 1 | 9995 | 135 | 600 |
| py-bugfix | 1 | cand | no | interrupted | 0 | 0 | 0 | 600 |
| py-config-cli | 1 | base | no | interrupted | 0 | 0 | 0 | 600 |
| py-config-cli | 1 | cand | no | interrupted | 1 | 10103 | 89 | 600 |
| py-ledger | 1 | base | no | interrupted | 0 | 0 | 0 | 600 |
| py-ledger | 1 | cand | no | interrupted | 1 | 10179 | 267 | 600 |

Small samples: read the success CI before any token delta. A token saving that costs success is a regression (principle 1).
