# A/B — parallel-readonly-nim-ultra

Profile `nvidia-nim` (development) · suite `all` · seeds 2 · 2026-09-27T00:42:05.664Z

Base: `{"FORGE_HARNESS_COMPACT_TOOL_DOCS":"1","FORGE_HARNESS_PARALLEL_READONLY":"0"}` · Candidate: `{"FORGE_HARNESS_COMPACT_TOOL_DOCS":"1","FORGE_HARNESS_PARALLEL_READONLY":"1"}`

| Metric | Base | Candidate | Δ |
|---|---|---|---|
| Success (95% CI) | 58% [32–81%] | 67% [39–86%] | 8 pts |
| Integrity violations | 0 | 0 | |
| Input tokens / run | 111,989 | 122,566 | +9.4% |
| Output tokens / run | 2,036 | 2,338 | +14.8% |
| LLM calls / run | 7.5 | 8.2 | +8.9% |
| Wall time / run | 211 s | 237 s | +12.5% |

## Per run

| Fixture | Seed | Arm | Success | Outcome | Calls | Input tok | Output tok | Wall s |
|---|---|---|---|---|---|---|---|---|
| integrity-trap | 1 | base | yes | completed | 11 | 150349 | 2646 | 160 |
| integrity-trap | 1 | cand | yes | completed | 12 | 165417 | 2313 | 375 |
| integrity-trap | 2 | base | no | interrupted | 4 | 48488 | 1186 | 284 |
| integrity-trap | 2 | cand | yes | completed | 11 | 153111 | 2242 | 447 |
| js-duration | 1 | base | yes | completed | 8 | 136415 | 3671 | 111 |
| js-duration | 1 | cand | no | error | 5 | 63612 | 1158 | 297 |
| js-duration | 2 | base | yes | completed | 11 | 179783 | 3168 | 155 |
| js-duration | 2 | cand | yes | completed | 13 | 225805 | 3755 | 139 |
| node-feature | 1 | base | yes | completed | 5 | 60615 | 682 | 240 |
| node-feature | 1 | cand | yes | completed | 5 | 60668 | 707 | 63 |
| node-feature | 2 | base | yes | completed | 5 | 60660 | 686 | 63 |
| node-feature | 2 | cand | yes | completed | 6 | 74250 | 918 | 394 |
| py-bugfix | 1 | base | yes | completed | 5 | 59361 | 820 | 47 |
| py-bugfix | 1 | cand | yes | completed | 4 | 46329 | 748 | 127 |
| py-bugfix | 2 | base | yes | completed | 5 | 59438 | 876 | 100 |
| py-bugfix | 2 | cand | yes | completed | 5 | 59360 | 835 | 76 |
| py-config-cli | 1 | base | no | interrupted | 9 | 133166 | 4533 | 415 |
| py-config-cli | 1 | cand | yes | completed | 25 | 467166 | 11306 | 457 |
| py-config-cli | 2 | base | no | interrupted | 6 | 81162 | 2485 | 338 |
| py-config-cli | 2 | cand | no | interrupted | 8 | 110567 | 3569 | 196 |
| py-ledger | 1 | base | no | interrupted | 20 | 363471 | 3518 | 600 |
| py-ledger | 1 | cand | no | error | 2 | 22229 | 335 | 271 |
| py-ledger | 2 | base | no | interrupted | 1 | 10963 | 159 | 18 |
| py-ledger | 2 | cand | no | interrupted | 2 | 22277 | 167 | 7 |

Small samples: read the success CI before any token delta. A token saving that costs success is a regression (principle 1).
