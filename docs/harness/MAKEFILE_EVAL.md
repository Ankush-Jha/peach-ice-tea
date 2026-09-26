# Makefile-based evaluation contract (organizer document, 2026-09-26)

Source: `~/Downloads/AI Harness Submission.md`, supplied by the team. Treated as authoritative
alongside `HACKATHON.md`; where the two differ on submission mechanics, this document is more
specific and wins for that mechanics.

## What it requires, in full

1. **A `Makefile` at the repository root** exposing `make setup`, `make run`, `make test`, and
   `make clean`. Minimum bar: `make setup` then `make run` must both succeed from a clean clone.
2. **Credential via `AI_API_KEY`**, exported by the evaluator before `make setup`. Never
   hard-coded anywhere committed (source, Makefile, `.env`, docs, config). A `.env.example` with
   an empty `AI_API_KEY=` is fine; a real key committed anywhere is not.
3. **Text-only model.** No image/audio/video/multimodal requirement — already true of this
   harness.
4. **Model is whatever the Organising Committee prescribes**, set at evaluation time, without
   modifying submitted source. The credential name (`AI_API_KEY`) is fixed regardless of which
   provider that turns out to be.
5. **Evaluator workflow:** `git clone` → `export AI_API_KEY=...` → `make setup` → `make run` →
   the issue/test case is supplied to the *running* harness → optionally `make test`.
6. **TUI teams:** must be reachable through `make run` alone, no team-specific discovery.
7. **No modification on their side.** If the Makefile interface doesn't work standalone, that's
   scored against the submission, not fixed by the evaluator.

## Open question this raises (flagging, not guessing past it)

Step 5 implies the harness stays *running* after `make run` and then receives the issue —
which reads like an interactive/TUI shape. This harness's actual model is `HACKATHON.md`'s
one-shot: a single frozen prompt, then `peach exec` runs to completion and exits. There is no
"launch, then feed it a task" mode today.

**Resolution to implement, defensively, without waiting for organizer clarification:**
`make run` should accept the task two ways at once, so either evaluator behavior works:
- **piped/typed stdin** (`make run < issue.txt`, or typed interactively and closed with EOF) —
  matches "launch, then supply the issue" most literally;
- **`make run PROMPT="..."`** as a documented convenience for scripted evaluation.

Both paths lead into the existing one-shot `peach exec` path (`harness/peach-ice-tea`), not a
new execution mode. If the organizers later say explicitly that the harness must stay resident
and accept multiple inputs across a session, that is new scope and needs a real decision, not
a guess baked into the Makefile silently.

## `AI_API_KEY` mapping

`harness/peach-ice-tea` and its profiles (`configuration/profiles/*`) each read a
provider-specific variable (`GEMINI_API_KEY`, `OPENROUTER_API_KEY`, ...). The organizer contract
fixes the *name* `AI_API_KEY` but not the *provider* — that's told to teams at evaluation time
per §4. The root `Makefile` is the one place that reconciles this: it exports `AI_API_KEY` as
whatever variable the currently-selected profile expects, so source code and profiles never
change when the prescribed model is announced — only the Makefile's profile selection does.

Default profile until told otherwise: **OpenRouter, free-tier model** (see the cost decision in
`DECISIONS.md`) — OpenRouter can front nearly any vendor's model behind one contract, so it is
the least-churn default if/when the organizers name a specific model family.
