# Per-tool error rate by model — 2026-09-27

`benchmarks/hackathon/tool_errors.ts` over every real run report to date (round 1, the free-tier screen, the Nemotron smoke run). Errors/calls from peach's own metrics; the model from each run's telemetry (D-079). Runs refused before any tool call are left out.

| Model | fs_search | patch | read | shell | todo_write |
|---|---|---|---|---|---|
| `nvidia/nemotron-3-ultra-550b-a55b:free` | 0/4 | 0/10 | 0/33 | 0/17 | 0/3 |
| `z-ai/glm-5.3` | – | 0/5 | 0/21 | 0/6 | – |
| `~deepseek/deepseek-pro-latest` | 0/1 | 0/2 | 1/13 (8%) | 0/2 | – |
| `dots-studio/dots-3-note-preview:free` | – | 0/1 | 0/2 | 0/1 | – |
| `cohere/north-mini-code:free` | – | 0/1 | 0/3 | 0/2 | – |
| `nvidia/nemotron-3-super-120b-a12b:free` | – | 0/1 | 0/2 | 0/1 | – |
| `nvidia/nemotron-3-nano-omni-30b-a3b-reasoning:free` | 0/2 | 0/1 | 0/4 | – | – |
| `poolside/laguna-s-2.1:free` | – | 0/1 | 0/2 | 0/1 | – |
| `nvidia/nemotron-3.5-lightning:free` | – | 1/2 (50%) | 1/4 (25%) | 0/1 | – |
