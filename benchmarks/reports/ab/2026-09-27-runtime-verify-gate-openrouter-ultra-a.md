# A/B — runtime-verify-gate-openrouter-ultra-a

Profile `openrouter` (evaluation) · suite `py-bugfix,node-feature` · seeds 1 · 2026-09-27T03:23:47.795Z

Base: `{"FORGE_RUNTIME_VERIFY_GATE":"false"}` · Candidate: `{"FORGE_RUNTIME_VERIFY_GATE":"true"}`

| Metric | Base | Candidate | Δ |
|---|---|---|---|
| Success (95% CI) | 100% [34–100%] | 100% [34–100%] | 0 pts |
| Integrity violations | 0 | 0 | |
| Input tokens / run | 60,690 | 60,640 | -0.1% |
| Output tokens / run | 801 | 802 | +0.1% |
| LLM calls / run | 5.0 | 5.0 | +0.0% |
| Wall time / run | 24 s | 20 s | -15.0% |

## Per run

| Fixture | Seed | Arm | Success | Outcome | Calls | Input tok | Output tok | Wall s |
|---|---|---|---|---|---|---|---|---|
| node-feature | 1 | base | yes | completed | 5 | 61180 | 720 | 31 |
| node-feature | 1 | cand | yes | completed | 5 | 61151 | 713 | 24 |
| py-bugfix | 1 | base | yes | completed | 5 | 60199 | 882 | 17 |
| py-bugfix | 1 | cand | yes | completed | 5 | 60128 | 891 | 16 |

Small samples: read the success CI before any token delta. A token saving that costs success is a regression (principle 1).
