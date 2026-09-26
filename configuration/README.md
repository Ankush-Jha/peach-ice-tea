# configuration/

| Path | Purpose |
|---|---|
| `prompt-template.md` | The frozen-prompt template the eval runner fills from an issue (covers every HACKATHON §11 item). |
| `profiles/gemini/` | **Evaluation** profile, the only one `harness/peach-ice-tea` uses: `gemini-3.8-flash` at high effort. |
| `profiles/deepseek/` | Development only: DeepSeek's own API (`deepseek-flash`). |
| `profiles/nvidia-deepseek/` | Development only: DeepSeek V4.1 Flash on NVIDIA NIM. |

Each profile is a `forge.toml` (provider, model) plus a `profile.env` (non-secret settings and
`PROFILE_KEY_VAR`, the key variable it needs). Keys are never stored here. Experimental behaviours are
environment flags, all **off** by default until an A/B supports them: `FORGE_HARNESS_COMPACT_TOOL_DOCS`,
`FORGE_HARNESS_LINE_NUMBERS_OFF`, `FORGE_HARNESS_PARALLEL_READONLY`, `FORGE_HARNESS_TOOL_CORRECTION`,
`FORGE_HARNESS_HANDOFF_NOTE` (D-063).
Tier 1 behaviours that are on in `exec`: the verify gate (`FORGE_HARNESS_VERIFY_GATE=0` disables it) and
recovery hints (`FORGE_HARNESS_RECOVERY_HINTS=0` disables them).
