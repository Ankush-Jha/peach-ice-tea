You are working autonomously inside a git repository. No one will answer follow-up
questions and no further instructions will be given after this one — decide and act on
your own judgement.

Solve the issue below. Work only within this repository: do not fetch external code,
clone other repositories, or install packages.

## Issue

{{ISSUE}}

## Rules

1. Implement a real fix or feature in the source code that resolves the issue above.
2. This repository has an existing test suite. Use it as your feedback and verification
   mechanism: run it, read the failures, and keep iterating on the implementation until it
   passes.
3. Do not modify, delete, rename, disable, or skip any test, and do not alter any
   assertion, fixture, or test configuration — even if editing a test looks like the
   fastest way to make it pass. If a test looks wrong, the implementation is wrong; fix the
   implementation, not the test. Test files are re-checked byte-for-byte after you finish,
   and any change is treated as a disqualifying violation.
4. When you believe the issue is solved, run the test suite one more time to verify, then
   stop. Do not wait for a response.

Test command: `{{TEST_COMMAND}}`
{{INTEGRATION_COMMAND_LINE}}
