# LCC × DevClub — AI Coding Harness Hackathon: Master Specification

> Source of truth for what this project is ultimately judged on. Supplied by the organizers (status: **Draft**).
> Where this document and `SPEC.md` disagree, **this document wins**; see `ALIGNMENT.md` for how the
> backlog was re-prioritised against it.

| Field | Value |
|---|---|
| Status | Draft |
| Organizers | LCC × DevClub |
| Foundation model | **Gemini 3.8 High** (same for every team) |
| Results | ~1–2 days after the evaluation round |

## 1. Overview
An agent-engineering competition: teams build the **harness** around a fixed foundation model — the system that
orchestrates the model, manages context, interacts with the repository, invokes tools, recovers from failures and
completes software-engineering tasks. Same model, repository, issues, tests and evaluation conditions for all.
Long-term goal: find at least one repository worth continuing as a serious coding-agent project.

## 2. Vision
A harness determines: what information the model receives; how repository information is discovered; which tools
exist; when/how tools are invoked; how context is maintained and compressed; how work is planned and executed; how
failures are detected and recovered from; when the task is complete; how efficiently tokens and compute are used.
Focus is agent infrastructure and orchestration, not prompt engineering alone.

## 3. Problem statement
Build an autonomous coding-agent harness around the standardized model that can understand SE tasks, navigate an
existing repository, use tools intelligently, manage context, orchestrate model interactions, recover from failures,
and produce correct, verified changes with efficient use of resources.

## 4. What is built ("Model B")
May include: orchestration, planning, context management, memory/state, repo exploration, file ops, code search,
terminal, testing, error handling, retry/recovery, verification, tool routing, custom loops, sub-agents, context
compression, token optimisation. Architecture is open-ended.

## 5. Philosophy
**Standardize the environment and evidence, not the implementation.** Same model, repository, issues, unit tests,
integration tests, rules, telemetry protocol, reporting protocol, procedure. Free: orchestration, architecture,
context strategy, tools, memory, planning, recovery, token optimisation, internals.

## 6. Environment
Same model / repo / issues / tests / rules / telemetry / report schema / process. Exact API config, runtime limits and
infrastructure finalised before the event.
**External models: [TBD — organizer decision].** Recommended rule: evaluation restricted to the standardized model.

## 7. Evaluation repository
Identical existing codebase with source, unit tests, integration tests, config, and clearly defined issues.

## 8. Testing rules
Harnesses must not modify, delete, rename, disable or skip tests, alter assertions, manipulate test configuration, or
otherwise change the testing mechanism. The implementation must satisfy the provided tests; using tests as a feedback
and verification mechanism is encouraged. **Test integrity is verified before and after the run by inspecting and
comparing files. Violation ⇒ disqualification or significant penalty.**

## 9. Live evaluation
Three parallel panels of 3–4 technical teachers. Flow: assigned to panel → receive repo + issue → inspect → write
prompt → **prompt frozen** → harness executes autonomously → transcript + telemetry captured → tests and repo state
evaluated → standard report generated → panel reviews evidence → technical questioning → score.

## 10. One-shot rule
One attempt, one user-level prompt. Once execution begins: prompt frozen; harness cannot be modified; no manual
intervention, restart, or extra instructions. One-shot ≠ one model call — any number of internal calls, tool calls,
plans, test runs, retries and recoveries are allowed.

## 11. Prompt requirements
The prompt must instruct the harness to: solve the issue; work within the repo; use existing tests for verification;
not modify or manipulate tests; implement the solution; verify it. Organizers may supply a baseline test-integrity
instruction. The prompt becomes part of the transcript.

## 12. Typical execution
Task → understand → explore repo → plan → use tools → modify implementation → run tests → analyse failures →
recover/revise → verify → complete. Sequence is the team's choice.

## 13. Evidence
13.1 **Final outcome** — unit tests, integration tests, repository state, correctness, issue requirements.
13.2 **Transcript** — prompt, model calls, tool calls, tool results, context changes, agent state changes, errors,
retries, test execution, final response.
13.3 **Telemetry** — token usage, context usage, tool usage, model calls, execution time, retries, errors, other
standardized metrics.
13.4 **Standardized report** generated from telemetry and execution data.
13.5 **Technical interview.**

## 14. Telemetry protocol (supplied by organizers)
`telemetry/telemetry.md` (protocol, event types, required fields, metric definitions, integration),
`telemetry/telemetryAgent.md` (agent-facing recording instructions), `telemetry/telemetry.schema.json`
(machine-readable event structure).

## 15. Telemetry requirements (where applicable)
- **Model:** invocation, input/output/total tokens, context size, timestamp, model-call id.
- **Context:** size, composition, updates, compression, retained state.
- **Tools:** name, invocation, arguments, result, success/failure, execution time.
- **Agent:** state, orchestration transitions, retries, errors, recovery actions.
- **Testing:** test execution, result, execution time.
- **Execution:** start, end, total duration.

## 16. Telemetry integrity
Canonical telemetry files must remain unchanged (protocol id, version, checksum; may be verified). Telemetry is the
standardized logger. **Objective values such as token counts should come from the execution layer, not from
model-generated claims.**

## 17. Reporting package (supplied by organizers)
`reporting/reportCreating.md`, `reporting/reportCreatorAgent.md`, `reporting/report.schema.json`.

## 18. Standard report
Concise: task outcome, execution summary, model usage, token usage, context management, tool usage, orchestration,
error recovery, testing, repository changes, execution timeline. Objective evidence and derived metrics only — no
subjective claims. It is evidence, not the score.

## 19. Token efficiency
Useful work relative to resources, not lowest token count. Input/output/total tokens, tokens per call, context size,
repeated context, model calls, tool calls, retries, execution time. **A harness that uses fewer tokens but fails is
not more efficient than one that reliably completes.**

## 20. Context management
Relevance, growth, redundancy, repository retrieval, retention of important information, compression, state/memory,
avoiding unnecessary transmission. Goal: the right information at the right time, not minimum size.

## 21. Tool engineering
Usefulness, interface design, granularity, reliability, error handling, selection, sequencing, redundant calls, output
quality, recovery from tool failures. More tools ≠ better.

## 22. Orchestration
Loop design, planning, state management, decomposition, tool routing, verification, termination conditions,
adaptation, failure handling, internal iteration, coherence. Architecture not prescribed.

## 23. Error recovery & reliability
Responses to: failed command, failed tests, tool error, incorrect assumption, introduced regression, unusable model
action. *Can the harness identify failure, reason about it, adapt, and continue without human intervention?*

## 24. Repository & code quality
Solves the issue; fits existing architecture; avoids unnecessary changes; preserves behaviour; appropriate
abstractions; no unnecessary dependencies; maintainable. Behave like an engineer in an existing codebase, not
"generate until tests pass".

## 25. Technical interview (sample questions)
- **Orchestration:** how the next step is decided; how completion is decided; what happens when stuck; why this architecture.
- **Context:** what enters context; how growth is prevented; what is compressed/discarded; how state is retained.
- **Tools:** why these tools; how the model chooses; tool failures; why this interface.
- **Tokens:** where tokens go; avoiding repetition; what controls context size; token vs reasoning trade-offs.
- **Reliability:** failing tests; distinguishing failure types; recovering from wrong implementations.
- **Architecture:** major components; most important decisions; what you would change next cycle.
Questions are grounded in the actual transcript and telemetry.

## 26. Rubric (proposed; may change after pilot)
| Category | Weight |
|---|---|
| Task completion & correctness | 30 |
| Orchestration & agent architecture | 15 |
| Context management | 15 |
| Tool design & usage | 10 |
| Token / resource efficiency | 10 |
| Error recovery & reliability | 10 |
| Code / system quality | 5 |
| Technical understanding | 5 |
Correctness is weighted highest; sophistication that does not solve tasks should not beat simplicity that does.

## 27–29. Panels and fairness
Three parallel panels (~20–30 teams total), identical procedure, repo, issue, model, rules, telemetry, reporting and
rubric. Procedure: assignment → **environment verification** → task → prompt → freeze → execution → evidence
collection → questioning → scoring → lock. Controlled: model, repo, issues, tests, rules, telemetry, report format,
one-shot, procedure, rubric. Open: architecture, orchestrator, tools, context, memory, planning, recovery, internals.

## 30. Evidence record
Team ID, harness version, evaluation protocol version, report protocol version, issue, prompt, final repository state,
test results, transcript, telemetry, standard report, panel score.

## 31. Prohibited
Manual intervention; changing the prompt after start; modifying the harness during evaluation; modifying or bypassing
tests; manipulating test results; tampering with telemetry; manipulating the report; unauthorized external models;
unauthorized external services; altering the evaluation environment. Final rules on external services, internet
access and extra models to be published before the event.

## 32. Submission
A complete, runnable harness repository, at minimum:
```
repository/
├── harness/
├── telemetry/      (telemetry.md, telemetryAgent.md, telemetry.schema.json)
├── reporting/      (reportCreating.md, reportCreatorAgent.md, report.schema.json)
├── README.md
├── configuration/
└── documentation/
```
README must cover: setup, execution, architecture, dependencies, configuration, major design decisions, known limitations.

## 33–35. Results and success
Leaderboard after all evaluations, with a 1–2 day verification window. Leading repositories may be reviewed for
architecture, maintainability, extensibility, documentation, reproducibility, performance, engineering quality and
potential for continued development.

## 36. Design principles
1 Same model, different harnesses · 2 Transparent evaluation · 3 Evidence over claims · 4 Human judgement where it
matters · 5 Objective measurement where possible · 6 Freedom of architecture · 7 Correctness first ·
8 Efficiency matters · 9 No test manipulation · 10 Build something worth keeping.
