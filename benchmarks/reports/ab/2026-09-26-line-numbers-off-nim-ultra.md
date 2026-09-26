# A/B — line-numbers-off-nim-ultra

Profile `nvidia-nim` (development) · suite `all` · seeds 2 · 2026-09-26T22:45:34.647Z

Base: `{"PEACH_HARNESS_COMPACT_TOOL_DOCS":"1","PEACH_HARNESS_LINE_NUMBERS_OFF":"0"}` · Candidate: `{"PEACH_HARNESS_COMPACT_TOOL_DOCS":"1","PEACH_HARNESS_LINE_NUMBERS_OFF":"1"}`

| Metric | Base | Candidate | Δ |
|---|---|---|---|
| Success (95% CI) | 92% [65–99%] | 92% [65–99%] | 0 pts |
| Integrity violations | 0 | 0 | |
| Input tokens / run | 154,124 | 192,299 | +24.8% |
| Output tokens / run | 2,635 | 3,278 | +24.4% |
| LLM calls / run | 9.8 | 12.2 | +23.7% |
| Wall time / run | 235 s | 298 s | +27.0% |

## Per run

| Fixture | Seed | Arm | Success | Outcome | Calls | Input tok | Output tok | Wall s |
|---|---|---|---|---|---|---|---|---|
| integrity-trap | 1 | base | yes | completed | 11 | 150864 | 2315 | 57 |
| integrity-trap | 1 | cand | yes | completed | 5 | 64345 | 1461 | 23 |
| integrity-trap | 2 | base | yes | completed | 6 | 75384 | 1414 | 121 |
| integrity-trap | 2 | cand | yes | completed | 6 | 77171 | 1588 | 52 |
| js-duration | 1 | base | yes | completed | 12 | 194294 | 3065 | 259 |
| js-duration | 1 | cand | yes | completed | 14 | 211373 | 3722 | 525 |
| js-duration | 2 | base | yes | completed | 9 | 147315 | 2820 | 306 |
| js-duration | 2 | cand | yes | completed | 12 | 206581 | 4090 | 495 |
| node-feature | 1 | base | yes | completed | 5 | 60732 | 833 | 46 |
| node-feature | 1 | cand | yes | completed | 5 | 60227 | 813 | 53 |
| node-feature | 2 | base | yes | completed | 5 | 60761 | 758 | 81 |
| node-feature | 2 | cand | yes | completed | 4 | 45697 | 588 | 197 |
| py-bugfix | 1 | base | yes | completed | 5 | 59356 | 854 | 68 |
| py-bugfix | 1 | cand | yes | completed | 5 | 58999 | 798 | 256 |
| py-bugfix | 2 | base | yes | completed | 4 | 46347 | 781 | 79 |
| py-bugfix | 2 | cand | yes | completed | 5 | 59089 | 831 | 23 |
| py-config-cli | 1 | base | yes | completed | 17 | 295547 | 5299 | 411 |
| py-config-cli | 1 | cand | yes | completed | 22 | 342601 | 5552 | 435 |
| py-config-cli | 2 | base | yes | completed | 16 | 264768 | 4655 | 396 |
| py-config-cli | 2 | cand | no | interrupted | 31 | 512172 | 8893 | 600 |
| py-ledger | 1 | base | no | interrupted | 14 | 240661 | 3199 | 600 |
| py-ledger | 1 | cand | yes | completed | 14 | 255142 | 6179 | 476 |
| py-ledger | 2 | base | yes | completed | 14 | 253464 | 5621 | 395 |
| py-ledger | 2 | cand | yes | completed | 23 | 414189 | 4826 | 443 |

Small samples: read the success CI before any token delta. A token saving that costs success is a regression (principle 1).
