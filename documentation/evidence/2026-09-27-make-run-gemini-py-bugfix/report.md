# Run report — peach-ice-tea 0.1.0

| Outcome | Exit | Model | Wall time |
|---|---|---|---|
| error | 1 | gemini-3.8-flash | 207.3 s |

Error: `provider quota exhausted (GenerateRequestsPerDayPerProjectPerModel-FreeTier), not retried: POST https://generativelanguage.googleapis.com/v1beta/models/gemini-3.8-flash:streamGenerateContent?alt=sse: Invalid Status Code: 429`

## Tokens (provider-reported)

| Input | Cached input | Output | Reasoning | Cache hit rate |
|---|---|---|---|---|
| 345977 | 198671 | 7436 | 6251 | 57.4% |

## Model calls

| Calls | Failed | Retried attempts (metrics) | Retry events (telemetry) | Input tok/call (min/mean/max) | Context tok est. (min/mean/max) | Duration ms (min/mean/max) |
|---|---|---|---|---|---|---|
| 18 | 1 | 15 | 15 | 11644 / 18479 / 23033 | 12637 / 15807 / 17978 | 1449 / 11402 / 53730 |

## Context

Compactions: 0 · tokens reclaimed (estimated): 0

First request, estimated tokens by source: system_prompt 2902 (23%) · tool_definitions 9497 (75%) · user_prompt 238 (2%)

## Tools

| Tool | Calls | Errors | Total ms |
|---|---|---|---|
| fs_search | 1 | 0 | 4 |
| patch | 1 | 0 | 1164 |
| read | 3 | 0 | 0 |
| shell | 6 | 0 | 180 |
| todo_read | 1 | 0 | 0 |
| todo_write | 6 | 0 | 1 |

## Error recovery

Retries: empty_completion ×1, http_429 ×12, http_503 ×2 · unmetered empty completions: 0 · tool errors: 0 · refused test edits: 0 · suppressed prompts: 0

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

- `2026-09-26T20:24:36.786Z` run started
- `2026-09-26T20:24:36.903Z` [peach] agent running
- `2026-09-26T20:24:36.909Z` [peach] prompt composition recorded
- `2026-09-26T20:24:41.588Z` [peach] retry 1 of model_call: http_503
- `2026-09-26T20:24:58.857Z` [peach] model call: 11644 in / 308 out, 1 tool call(s), finish end_turn
- `2026-09-26T20:24:58.881Z` [peach] tool todo_write: ok
- `2026-09-26T20:25:01.533Z` [peach] model call: 12113 in / 202 out, 2 tool call(s), finish end_turn
- `2026-09-26T20:25:01.534Z` [peach] tool read: ok
- `2026-09-26T20:25:01.534Z` [peach] tool read: ok
- `2026-09-26T20:25:06.728Z` [peach] model call: 12983 in / 766 out, 1 tool call(s), finish end_turn
- `2026-09-26T20:25:06.729Z` [peach] tool todo_write: ok
- `2026-09-26T20:25:09.435Z` [peach] model call: 13888 in / 38 out, 1 tool call(s), finish end_turn
- `2026-09-26T20:25:09.501Z` test run: exit Some(1)
- `2026-09-26T20:25:09.501Z` [peach] tool shell: ok
- `2026-09-26T20:25:12.389Z` [peach] retry 1 of model_call: empty_completion
- `2026-09-26T20:25:14.478Z` [peach] retry 2 of model_call: http_429
- `2026-09-26T20:25:17.038Z` [peach] retry 3 of model_call: http_429
- `2026-09-26T20:25:24.826Z` [peach] retry 4 of model_call: http_429
- `2026-09-26T20:25:36.949Z` [peach] retry 5 of model_call: http_429
- `2026-09-26T20:25:58.186Z` [peach] model call: 14308 in / 67 out, 1 tool call(s), finish end_turn
- `2026-09-26T20:25:58.212Z` [peach] tool shell: ok
- `2026-09-26T20:26:00.460Z` [peach] model call: 14564 in / 40 out, 1 tool call(s), finish end_turn
- `2026-09-26T20:26:00.464Z` [peach] tool fs_search: ok
- `2026-09-26T20:26:13.703Z` [peach] model call: 14837 in / 3771 out, 1 tool call(s), finish end_turn
- `2026-09-26T20:26:13.723Z` [peach] tool shell: ok
- `2026-09-26T20:26:17.605Z` [peach] retry 1 of model_call: http_503
- `2026-09-26T20:26:24.274Z` [peach] model call: 19381 in / 1132 out, 1 tool call(s), finish end_turn
- `2026-09-26T20:26:24.275Z` [peach] tool todo_write: ok
- `2026-09-26T20:26:26.394Z` [peach] model call: 20603 in / 266 out, 1 tool call(s), finish end_turn
- `2026-09-26T20:26:27.559Z` [peach] tool patch: ok
- `2026-09-26T20:26:27.930Z` [peach] retry 1 of model_call: http_429
- `2026-09-26T20:26:29.641Z` [peach] retry 2 of model_call: http_429
- `2026-09-26T20:26:33.439Z` [peach] retry 3 of model_call: http_429
- `2026-09-26T20:26:40.676Z` [peach] model call: 21106 in / 76 out, 1 tool call(s), finish end_turn
- `2026-09-26T20:26:40.677Z` [peach] tool todo_write: ok
- `2026-09-26T20:26:44.420Z` [peach] model call: 21318 in / 39 out, 1 tool call(s), finish end_turn
- `2026-09-26T20:26:44.461Z` test run: exit Some(0)
- `2026-09-26T20:26:44.462Z` [peach] tool shell: ok
- `2026-09-26T20:26:51.245Z` [peach] model call: 21471 in / 57 out, 1 tool call(s), finish end_turn
- `2026-09-26T20:26:51.260Z` [peach] tool shell: ok
- `2026-09-26T20:26:52.717Z` [peach] model call: 21766 in / 31 out, 1 tool call(s), finish end_turn
- `2026-09-26T20:26:52.735Z` [peach] tool shell: ok
- `2026-09-26T20:26:54.422Z` [peach] model call: 21957 in / 131 out, 1 tool call(s), finish end_turn
- `2026-09-26T20:26:54.423Z` [peach] tool read: ok
- `2026-09-26T20:26:55.968Z` [peach] model call: 22329 in / 40 out, 1 tool call(s), finish end_turn
- `2026-09-26T20:26:55.968Z` [peach] tool todo_write: ok
- `2026-09-26T20:26:56.409Z` [peach] retry 1 of model_call: http_429
- `2026-09-26T20:26:58.596Z` [peach] retry 2 of model_call: http_429
- `2026-09-26T20:27:01.638Z` [peach] retry 3 of model_call: http_429
- `2026-09-26T20:27:05.987Z` [peach] retry 4 of model_call: http_429
- `2026-09-26T20:27:22.304Z` [peach] retry 5 of model_call: http_429
- `2026-09-26T20:27:49.707Z` [peach] model call: 22455 in / 368 out, 0 tool call(s), finish end_turn
- `2026-09-26T20:27:49.711Z` [peach] agent ended
- `2026-09-26T20:27:56.549Z` [peach] model call: 22871 in / 29 out, 1 tool call(s), finish end_turn
- `2026-09-26T20:27:56.550Z` [peach] tool todo_read: ok
- `2026-09-26T20:28:03.676Z` [peach] model call: 23033 in / 75 out, 1 tool call(s), finish end_turn
- `2026-09-26T20:28:03.677Z` [peach] tool todo_write: ok
- `2026-09-26T20:28:04.060Z` integrity verify: 2 protected files checked; all unchanged
- `2026-09-26T20:28:04.142Z` test run: exit Some(0)
- `2026-09-26T20:28:04.161Z` error error: provider quota exhausted (GenerateRequestsPerDayPerProjectPerModel-FreeTier), not retried: POST https://generativelanguage.googleapis.com/v1beta/models/gemini-3.8-flash:streamGenerateContent?alt=sse: Invalid Status Code: 429
- `2026-09-26T20:28:04.161Z` run ended: error
