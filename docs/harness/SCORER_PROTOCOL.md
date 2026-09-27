# External relevance scorer protocol — `peach-ice-tea.scorer/1`

How an outside scorer, such as an adapter for save-token-jev's **Jev** model, plugs into compaction stage S2
(R-CTX-4, R-EXT-1; D-098). The harness never depends on it (D-003). With no scorer configured, S2 uses the built-in
`HeuristicScorer`.

## Turning it on

```bash
export PEACH_HARNESS_SCORE_STAGE=1                          # S2 itself (D-077; off by default)
export PEACH_HARNESS_EXTERNAL_SCORER="node my-jev-adapter.js"  # run through /bin/sh -c
export PEACH_HARNESS_EXTERNAL_SCORER_TIMEOUT_SECS=30        # optional; default 30
```

The command inherits the harness's environment, so an adapter reads its own key (for example `JEV_API_KEY`) from
there. The harness never passes model-provider keys to it on purpose, and never writes any key anywhere.

## Request (stdin, one JSON object)

```json
{
  "protocol": "peach-ice-tea.scorer/1",
  "goal": "the task, from the first user message",
  "threshold": 0.5,
  "calls": [
    {
      "id": "m12",
      "tool": "shell",
      "input_preview": "{\"command\":\"pytest -x\"}",
      "status": "error",
      "result_chars": 18421,
      "message_index": 12,
      "referenced_later": true
    }
  ]
}
```

- `calls` excludes pinned calls: the first message and the retention window never reach a scorer.
- `input_preview` is already redacted (R-SAFE-3).
- Results themselves are not sent, only their size and status. This is save-token-jev's own "state": `ok|error, N
  chars (omitted)`.

## Answer (stdout, one JSON object)

save-token-jev's answer shape, with one entry per call id:

```json
{ "m12": { "keep_call": 0.92, "keep_result": 0.35 } }
```

- `keep_call`: does knowing this call ran still matter?
- `keep_result`: must the full result stay verbatim, because re-running would not recover it?

These are the two questions of save-token-jev's `questionsFor`. The harness applies its `decide` rule at the
request's `threshold`:

| Answer | Decision |
|---|---|
| `keep_result` ≥ threshold | keep |
| `keep_call` ≥ threshold | truncate to a 300-character head |
| neither | drop |

## Guarantees the harness adds on top of Jev

- **Reversible:** a truncated or dropped result is offloaded to a file, and the stub in context says
  `read <path>`. A Jev decision can therefore never lose data here, which is stricter than save-token-jev's own
  hosts.
- **Fail-open (R-CTX-5):** the stage keeps everything if the scorer exits non-zero, times out, prints anything but
  the object above, or gives a probability outside 0–1. That matches save-token-jev's own "malformed answer aborts
  the attempt".
- **Unanswered calls are kept**, not dropped.
- **Measured:** each compaction's decisions land in telemetry (`context_compaction`) and in the run report, so a Jev
  scorer can be A/B-tested against the heuristic with `benchmarks/hackathon/ab.ts`, like any other flag.

## Minimal scorer (keeps everything)

```bash
#!/bin/sh
cat >/dev/null
echo '{}'
```
