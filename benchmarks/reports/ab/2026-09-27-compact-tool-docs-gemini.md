# A/B — compact-tool-docs-gemini

Profile `gemini` (evaluation) · suite `py-bugfix` · seeds 1 · 2026-09-27T03:22:06.384Z

Base: `{"PEACH_HARNESS_COMPACT_TOOL_DOCS":"0"}` · Candidate: `{"PEACH_HARNESS_COMPACT_TOOL_DOCS":"1"}`

| Metric | Base | Candidate | Δ |
|---|---|---|---|
| Success (95% CI) | 0% [0–79%] | 0% [0–79%] | 0 pts |
| Integrity violations | 0 | 0 | |
| Input tokens / run | 0 | 0 | n/a |
| Output tokens / run | 0 | 0 | n/a |
| LLM calls / run | 0.0 | 0.0 | n/a |
| Wall time / run | 1 s | 1 s | -11.7% |

## Per run

| Fixture | Seed | Arm | Success | Outcome | Calls | Input tok | Output tok | Wall s |
|---|---|---|---|---|---|---|---|---|
| py-bugfix | 1 | base | no | error | 0 | 0 | 0 | 1 |
| py-bugfix | 1 | cand | no | error | 0 | 0 | 0 | 1 |

Small samples: read the success CI before any token delta. A token saving that costs success is a regression (principle 1).
