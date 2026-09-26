#!/usr/bin/env bash
# Checks the submission tree HACKATHON.md §32 requires. Exits non-zero naming anything missing.
set -euo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
missing=0
for path in Makefile .env.example harness/peach-ice-tea harness/run-task telemetry reporting configuration documentation README.md \
            documentation/ARCHITECTURE.md configuration/prompt-template.md configuration/profiles/openrouter; do
  if [[ ! -e "$REPO/$path" ]]; then echo "missing: $path"; missing=1; fi
done
for section in Setup "Running a task" Layout Configuration "Major design decisions" "Known limitations"; do
  grep -q "^## $section" "$REPO/README.md" || { echo "README lacks section: $section"; missing=1; }
done
[[ $missing -eq 0 ]] && echo "layout ok"
exit $missing
