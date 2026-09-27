# A/B — doom-loop-escalation-nim-deepseek

Profile `nvidia-deepseek` (development) · suite `py-bugfix,node-feature,py-ledger` · seeds 1 · 2026-09-27T00:18:33.943Z

Base: `{"FORGE_HARNESS_COMPACT_TOOL_DOCS":"1","FORGE_HARNESS_DOOM_LOOP_ESCALATION":"0"}` · Candidate: `{"FORGE_HARNESS_COMPACT_TOOL_DOCS":"1","FORGE_HARNESS_DOOM_LOOP_ESCALATION":"1"}`

| Metric | Base | Candidate | Δ |
|---|---|---|---|
| Success (95% CI) | 100% [44–100%] | 100% [44–100%] | 0 pts |
| Integrity violations | 0 | 0 | |
| Input tokens / run | 58,755 | 74,077 | +26.1% |
| Output tokens / run | 1,645 | 1,624 | -1.3% |
| LLM calls / run | 4.7 | 5.7 | +21.4% |
| Wall time / run | 516 s | 528 s | +2.4% |

## Per run

| Fixture | Seed | Arm | Success | Outcome | Calls | Input tok | Output tok | Wall s |
|---|---|---|---|---|---|---|---|---|
| node-feature | 1 | base | yes | completed | 4 | 44849 | 544 | 128 |
| node-feature | 1 | cand | yes | completed | 4 | 44883 | 656 | 145 |
| py-bugfix | 1 | base | yes | completed | 4 | 44685 | 600 | 1131 |
| py-bugfix | 1 | cand | yes | completed | 4 | 44657 | 550 | 1131 |
| py-ledger | 1 | base | yes | completed | 6 | 86730 | 3791 | 288 |
| py-ledger | 1 | cand | yes | completed | 9 | 132691 | 3666 | 308 |

Small samples: read the success CI before any token delta. A token saving that costs success is a regression (principle 1).
