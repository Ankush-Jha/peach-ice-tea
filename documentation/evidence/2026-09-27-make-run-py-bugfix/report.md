# Run report — peach-ice-tea 0.1.0

| Outcome | Exit | Model | Wall time |
|---|---|---|---|
| completed | 0 | nvidia/nemotron-3-ultra-550b-a55b:free | 62.5 s |

## Tokens (provider-reported)

| Input | Cached input | Output | Reasoning | Cache hit rate |
|---|---|---|---|---|
| 66960 | 21600 | 870 | 218 | 32.3% |

## Model calls

| Calls | Failed | Retried attempts (metrics) | Retry events (telemetry) | Input tok/call (min/mean/max) | Context tok est. (min/mean/max) | Duration ms (min/mean/max) |
|---|---|---|---|---|---|---|
| 5 | 0 | 0 | 0 | 12185 / 13392 / 14172 | 12636 / 13487 / 14086 | 1700 / 12209 / 38275 |

## Context

Compactions: 0 · tokens reclaimed (estimated): 0

First request, estimated tokens by source: system_prompt 2901 (23%) · tool_definitions 9497 (75%) · user_prompt 238 (2%)

## Tools

| Tool | Calls | Errors | Total ms |
|---|---|---|---|
| patch | 1 | 0 | 930 |
| read | 2 | 0 | 2 |
| shell | 2 | 0 | 185 |

## Error recovery

Retries: none · unmetered empty completions: 0 · tool errors: 0 · refused test edits: 0 · suppressed prompts: 0

## Testing (the harness's own run after the agent stopped)

| Result | Passed | Failed | Skipped | Exit | Duration ms | Command | Why this command |
|---|---|---|---|---|---|---|---|
| passed | 4 | 0 | 0 | 0 | 53 | `python3 -m unittest discover -s tests -t . -v` | explicit |

Last 9 line(s) of output:

```
test_already_sorted_input (tests.test_stats.TestDedupeSorted.test_already_sorted_input) ... ok
test_empty_input (tests.test_stats.TestDedupeSorted.test_empty_input) ... ok
test_removes_duplicates_and_sorts (tests.test_stats.TestDedupeSorted.test_removes_duplicates_and_sorts) ... ok
test_single_value_repeated (tests.test_stats.TestDedupeSorted.test_single_value_repeated) ... ok

----------------------------------------------------------------------
Ran 4 tests in 0.000s

OK
```


## Repository changes

1 file(s), +1 −5

| File | + | − |
|---|---|---|
| `stats.py` | 1 | 5 |

## Test integrity

2 protected file(s) checked; all unchanged.

## Timeline

- `2026-09-26T18:38:40.182Z` run started
- `2026-09-26T18:38:40.453Z` [forge] agent running
- `2026-09-26T18:38:40.453Z` [forge] prompt composition recorded
- `2026-09-26T18:39:18.728Z` [forge] model call: 12185 in / 278 out, 2 tool call(s), finish tool_calls
- `2026-09-26T18:39:18.731Z` [forge] tool read: ok
- `2026-09-26T18:39:18.731Z` [forge] tool read: ok
- `2026-09-26T18:39:23.350Z` [forge] model call: 13057 in / 361 out, 1 tool call(s), finish tool_calls
- `2026-09-26T18:39:24.281Z` [forge] tool patch: ok
- `2026-09-26T18:39:25.984Z` [forge] model call: 13644 in / 72 out, 1 tool call(s), finish tool_calls
- `2026-09-26T18:39:26.045Z` test run: exit Some(1)
- `2026-09-26T18:39:26.045Z` recovery: recovery_hint
- `2026-09-26T18:39:26.045Z` [forge] tool shell: ok
- `2026-09-26T18:39:38.373Z` [forge] model call: 13902 in / 63 out, 1 tool call(s), finish tool_calls
- `2026-09-26T18:39:38.499Z` test run: exit Some(0)
- `2026-09-26T18:39:38.499Z` [forge] tool shell: ok
- `2026-09-26T18:39:42.631Z` [forge] model call: 14172 in / 96 out, 0 tool call(s), finish end_turn
- `2026-09-26T18:39:42.633Z` [forge] agent ended
- `2026-09-26T18:39:42.639Z` integrity verify: 2 protected files checked; all unchanged
- `2026-09-26T18:39:42.741Z` test run: exit Some(0)
- `2026-09-26T18:39:42.743Z` run ended: completed
