# configuration/

| Path | Purpose |
|---|---|
| `prompt-template.md` | The frozen-prompt template the eval runner fills from an issue (covers every HACKATHON §11 item). |
| `profiles/gemini/` | **Evaluation** profile, the only one `harness/peach-ice-tea` uses: `gemini-3.8-flash` at high effort. |
| `profiles/deepseek/` | Development only: DeepSeek's own API (`deepseek-flash`). |
| `profiles/nvidia-deepseek/` | Development only: DeepSeek V4.1 Flash on NVIDIA NIM. |

Each profile is a `peach.toml` (provider, model) plus a `profile.env` (non-secret settings and
`PROFILE_KEY_VAR`, the key variable it needs). Keys are never stored here. Experimental behaviours are
environment flags, all **off** by default until an A/B supports them: `PEACH_HARNESS_COMPACT_TOOL_DOCS`,
`PEACH_HARNESS_LINE_NUMBERS_OFF`, `PEACH_HARNESS_PARALLEL_READONLY`, `PEACH_HARNESS_TOOL_CORRECTION`,
`PEACH_HARNESS_HANDOFF_NOTE` (D-063), `PEACH_HARNESS_RECALL_HANDLES` (D-066), `PEACH_HARNESS_SEARCH_REGROUP`, `PEACH_HARNESS_NOISE_COMPRESSION` (D-073), `PEACH_HARNESS_OFFLOAD` (D-074).
Tier 1 behaviours that are on in `exec`: the verify gate (`PEACH_HARNESS_VERIFY_GATE=0` disables it) and
recovery hints (`PEACH_HARNESS_RECOVERY_HINTS=0` disables them). `PEACH_HARNESS_FALLBACK_MODELS` (set in a
profile's `profile.env`) lists models to fail over to on an exhausted quota or an outage (D-072).
