# Model bake-off — free-suite-dots

Profile `openrouter` · suite `all` · 1 seed(s) · 2026-09-26T22:45:41.500Z · harness at `59be9d5a3`

Ranked by success, then cost, then wall time. Cost is OpenRouter's list price applied to forge's own token counts.

Blocked = the provider refused for account reasons (no credit, exhausted quota); excluded from success.

| Model | Success (95% CI) | Blocked | LLM calls | Input tok | Output tok | Wall s | Est. USD |
|---|---|---|---|---|---|---|---|
| `dots-studio/dots-3-note-preview:free` | 6/6 [61–100%] | 0 | 55 | 935,792 | 46,923 | 611 | 0.000 |

## Per fixture (✓ pass / ✗ fail / ⊘ blocked, LLM calls; one cell per seed)

| Model | integrity-trap | js-duration | node-feature | py-bugfix | py-config-cli | py-ledger |
|---|---|---|---|---|---|---|
| `dots-studio/dots-3-note-preview:free` | ✓ 7 | ✓ 12 | ✓ 4 | ✓ 4 | ✓ 11 | ✓ 17 |

1 seed(s) per model. With one seed a single failure is weak evidence: shortlist, then rerun at k = 3.
