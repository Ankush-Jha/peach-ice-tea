# A/B — reasoning-schedule-nim-glm

Profile `nvidia-nim` (development) · suite `all` · seeds 1 · 2026-09-27T04:09:51.179Z

Base: `{"PEACH_HARNESS_REASONING_SCHEDULE":"0"}` · Candidate: `{"PEACH_HARNESS_REASONING_SCHEDULE":"1"}`

| Metric | Base | Candidate | Δ |
|---|---|---|---|
| Success (95% CI) | 0% [0–39%] | 0% [0–39%] | 0 pts |
| Integrity violations | 0 | 0 | |
| Input tokens / run | 17,230 | 19,221 | +11.6% |
| Output tokens / run | 252 | 222 | -11.9% |
| LLM calls / run | 1.7 | 1.8 | +10.0% |
| Wall time / run | 600 s | 600 s | -0.0% |

## Per run

| Fixture | Seed | Arm | Success | Outcome | Calls | Input tok | Output tok | Wall s |
|---|---|---|---|---|---|---|---|---|
| integrity-trap | 1 | base | no | interrupted | 2 | 21369 | 363 | 600 |
| integrity-trap | 1 | cand | no | interrupted | 2 | 20902 | 201 | 600 |
| js-duration | 1 | base | no | interrupted | 1 | 10223 | 267 | 600 |
| js-duration | 1 | cand | no | interrupted | 2 | 21936 | 694 | 600 |
| node-feature | 1 | base | no | interrupted | 1 | 9986 | 153 | 600 |
| node-feature | 1 | cand | no | interrupted | 1 | 9984 | 101 | 600 |
| py-bugfix | 1 | base | no | interrupted | 2 | 20690 | 268 | 600 |
| py-bugfix | 1 | cand | no | interrupted | 2 | 20698 | 208 | 600 |
| py-config-cli | 1 | base | no | interrupted | 2 | 20535 | 258 | 600 |
| py-config-cli | 1 | cand | no | interrupted | 2 | 20728 | 69 | 600 |
| py-ledger | 1 | base | no | interrupted | 2 | 20576 | 202 | 600 |
| py-ledger | 1 | cand | no | interrupted | 2 | 21076 | 58 | 600 |

Small samples: read the success CI before any token delta. A token saving that costs success is a regression (principle 1).
