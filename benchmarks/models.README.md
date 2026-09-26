# Model registry for evals

`models.csv` is the single place that defines which models every eval runs against. It exists so that
`R-EVAL-1`'s "at least two model families (one Anthropic, one OpenAI)" is a property of the suite rather
than something each `task.yml` re-states differently — before this file, the same model appeared under four
different naming conventions across `benchmarks/evals/*/task.yml`.

## Columns

| Column | Meaning |
|---|---|
| `family` | The A/B arm this row belongs to. Reports group by this. |
| `provider` | A Forge provider id (see `crates/forge_repo/src/provider/provider.json`). |
| `model` | The provider's model id, as that provider spells it. |

## Using it from a `task.yml`

Reference it as a source; paths resolve relative to the eval's own directory, and sources cross-product,
so this multiplies the eval's task rows by the model rows:

```yaml
sources:
  - csv: ../../models.csv
  - value:
      - task: "..."
run:
  - FORGE_SESSION__PROVIDER_ID={{provider}} FORGE_SESSION__MODEL_ID={{model}} forgee -p '{{task}}'
```

The `commit` and `suggest` subcommands read their own config rather than `session`, so those two evals use
`FORGE_COMMIT__PROVIDER_ID`/`FORGE_COMMIT__MODEL_ID` and `FORGE_SUGGEST__PROVIDER_ID`/`FORGE_SUGGEST__MODEL_ID`
respectively. Any `ForgeConfig` field is settable this way: `FORGE_<FIELD>`, with `__` separating nested
fields and a single `_` meaning nothing special.

## Why every row routes through OpenRouter

Both arms deliberately go through `open_router` rather than the native `anthropic` and `openai` providers.
The evals' `jq` validations assume the OpenAI wire shape (`.messages[].tool_calls[].function.name`), which
is what OpenRouter emits. Forge's native `anthropic` provider sends Anthropic-shaped `content` blocks with
`tool_use` instead, so those filters would match nothing and the eval would **pass or fail for the wrong
reason, silently** — a worse failure than an error. One key (`OPENROUTER_API_KEY`) also covers both arms.

If an arm ever needs a native provider, migrate its validations to `FORGE_AUTO_DUMP=json` first, whose
`Conversation`/`Context` structure is provider-agnostic and additionally carries tool results and the final
assistant message. See `docs/harness/RECON.md` §2 and `DECISIONS.md` D-012/D-013.

## Re-baselining

Changing a model version is a one-line edit here. Per `RECON.md` §5 (cross-cutting risk 4), record the change
in `DECISIONS.md` and re-run the baseline — later A/Bs are not comparable across a model change.
