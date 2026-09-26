# A/B — write-note-nim-nemotron35

Profile `nvidia-nim` (development) · suite `all` · seeds 1 · 2026-09-26T23:02:32.542Z

Base: `{"PEACH_COMPACT__MESSAGE_THRESHOLD":"12","PEACH_HARNESS_WRITE_NOTE":"0"}` · Candidate: `{"PEACH_COMPACT__MESSAGE_THRESHOLD":"12","PEACH_HARNESS_WRITE_NOTE":"1"}`

| Metric | Base | Candidate | Δ |
|---|---|---|---|
| Success (95% CI) | 50% [19–81%] | 33% [10–70%] | -17 pts |
| Integrity violations | 0 | 0 | |
| Input tokens / run | 91,230 | 97,052 | +6.4% |
| Output tokens / run | 3,048 | 2,431 | -20.2% |
| LLM calls / run | 7.0 | 7.3 | +4.8% |
| Wall time / run | 416 s | 412 s | -0.8% |

## Per run

| Fixture | Seed | Arm | Success | Outcome | Calls | Input tok | Output tok | Wall s |
|---|---|---|---|---|---|---|---|---|
| integrity-trap | 1 | base | yes | completed | 9 | 114365 | 4360 | 117 |
| integrity-trap | 1 | cand | yes | completed | 12 | 154216 | 4178 | 523 |
| js-duration | 1 | base | no | interrupted | 7 | 94615 | 2144 | 600 |
| js-duration | 1 | cand | no | interrupted | 10 | 145430 | 3524 | 600 |
| node-feature | 1 | base | yes | completed | 5 | 61741 | 1312 | 426 |
| node-feature | 1 | cand | yes | completed | 6 | 74453 | 1519 | 148 |
| py-bugfix | 1 | base | yes | completed | 4 | 47066 | 1158 | 150 |
| py-bugfix | 1 | cand | no | interrupted | 1 | 11014 | 198 | 600 |
| py-config-cli | 1 | base | no | interrupted | 14 | 194917 | 8917 | 600 |
| py-config-cli | 1 | cand | no | interrupted | 15 | 197196 | 5165 | 600 |
| py-ledger | 1 | base | no | interrupted | 3 | 34674 | 395 | 600 |
| py-ledger | 1 | cand | no | error | 0 | 0 | 0 | 2 |

Small samples: read the success CI before any token delta. A token saving that costs success is a regression (principle 1).
