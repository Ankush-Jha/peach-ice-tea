# Model bake-off — free-screen-1

Profile `openrouter` · suite `py-bugfix` · 1 seed(s) · 2026-09-26T18:51:42.618Z · harness at `03ac3d416`

Ranked by success, then cost, then wall time. Cost is OpenRouter's list price applied to peach's own token counts.

Blocked = the provider refused for account reasons (no credit, exhausted quota); excluded from success.

| Model | Success (95% CI) | Blocked | LLM calls | Input tok | Output tok | Wall s | Est. USD |
|---|---|---|---|---|---|---|---|
| `dots-studio/dots-3-note-preview:free` | 1/1 [21–100%] | 0 | 4 | 50,777 | 703 | 43 | 0.000 |
| `cohere/north-mini-code:free` | 1/1 [21–100%] | 0 | 6 | 68,946 | 1,370 | 46 | 0.000 |
| `nvidia/nemotron-3-super-120b-a12b:free` | 1/1 [21–100%] | 0 | 5 | 67,789 | 1,371 | 51 | 0.000 |
| `poolside/laguna-s-2.1:free` | 1/1 [21–100%] | 0 | 4 | 51,176 | 661 | 577 | 0.000 |
| `nvidia/nemotron-3-nano-omni-30b-a3b-reasoning:free` | 0/1 [0–79%] | 0 | 7 | 94,791 | 2,678 | 211 | 0.000 |
| `nvidia/nemotron-3.5-lightning:free` | 0/1 [0–79%] | 0 | 6 | 82,390 | 1,107 | 600 | 0.000 |

## Per fixture (✓ pass / ✗ fail / ⊘ blocked, LLM calls; one cell per seed)

| Model | py-bugfix |
|---|---|
| `dots-studio/dots-3-note-preview:free` | ✓ 4 |
| `cohere/north-mini-code:free` | ✓ 6 |
| `nvidia/nemotron-3-super-120b-a12b:free` | ✓ 5 |
| `poolside/laguna-s-2.1:free` | ✓ 4 |
| `nvidia/nemotron-3-nano-omni-30b-a3b-reasoning:free` | ✗ 7 (request_limit) |
| `nvidia/nemotron-3.5-lightning:free` | ✗ 6 (interrupted) |

1 seed(s) per model. With one seed a single failure is weak evidence: shortlist, then rerun at k = 3.
