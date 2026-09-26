# Peach Ice Tea — evaluator interface (docs/harness/MAKEFILE_EVAL.md, D-070).
#
#   export AI_API_KEY=...        # the only credential; never written anywhere
#   make setup                   # build the harness and install what it needs
#   make run REPO=/path/to/repo  # then type or pipe the issue (end with Ctrl-D)
#   make run REPO=... PROMPT="Fix ..."   # or give it inline
#   make test                    # the local hackathon suite, live (spends model requests)
#   make check                   # offline checks, no key and no model needed
#   make clean                   # remove build output and evidence
#
# Variables: PROFILE (default openrouter; see configuration/profiles/), MODEL (override the
# profile's model, e.g. the one the organisers prescribe), REPO, PROMPT, TEST_COMMAND,
# EVIDENCE_DIR, MAX_DURATION_SECS. AI_API_KEY is handed to the profile as the provider
# variable it expects, so changing provider never means changing source.

SHELL := /bin/bash
.SHELLFLAGS := -eu -o pipefail -c
.DEFAULT_GOAL := help

HARNESS := $(patsubst %/,%,$(dir $(abspath $(lastword $(MAKEFILE_LIST)))))
PROFILE ?= openrouter
BIN := $(HARNESS)/target/release/forge
# Where make was invoked from: `make -f <harness>/Makefile run` inside the target repo works.
REPO ?= $(CURDIR)

export PROFILE MODEL PROMPT REPO TEST_COMMAND EVIDENCE_DIR MAX_DURATION_SECS

.PHONY: help setup run test check clean

help:
	@sed -n '2,15p' $(HARNESS)/Makefile | sed 's/^# \{0,1\}//'

setup:
	@command -v cargo >/dev/null || { \
	  echo "setup: installing the Rust toolchain (rustup)"; \
	  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path; }
	@command -v protoc >/dev/null || { \
	  if command -v brew >/dev/null; then echo "setup: installing protoc (brew)"; brew install protobuf; \
	  else echo "setup: protoc is required: apt-get install -y protobuf-compiler (or see docs/harness/DEV.md)" >&2; exit 1; fi; }
	@command -v node >/dev/null || { echo "setup: Node.js >= 22 is required for make test (https://nodejs.org)" >&2; exit 1; }
	@PATH="$$HOME/.cargo/bin:$$PATH" $(HARNESS)/harness/build.sh
	@cd $(HARNESS) && npm ci --no-audit --no-fund --loglevel=error
	@$(HARNESS)/harness/check-layout.sh
	@echo "setup: done. Next: export AI_API_KEY=... && make run REPO=/path/to/repository"

run:
	@test -x $(BIN) || { echo "run: no harness binary; run make setup first" >&2; exit 1; }
	@PEACH_ICE_TEA_BIN=$(BIN) $(HARNESS)/harness/run-task

test:
	@test -x $(BIN) || { echo "test: no harness binary; run make setup first" >&2; exit 1; }
	@test -n "$${AI_API_KEY:-}" || { echo "test: AI_API_KEY is not set (make check needs no key)" >&2; exit 1; }
	@KEY_VAR=$$(sed -n 's/^PROFILE_KEY_VAR=//p' $(HARNESS)/configuration/profiles/$(PROFILE)/profile.env); \
	  export "$$KEY_VAR=$$AI_API_KEY"; unset AI_API_KEY; \
	  if [ -n "$${MODEL:-}" ]; then export FORGE_SESSION__MODEL_ID="$$MODEL"; fi; \
	  cd $(HARNESS) && node benchmarks/hackathon/run.ts --agent forge --profile $(PROFILE) --bin $(BIN) --suite all

check:
	@cd $(HARNESS) && npx tsx --test benchmarks/hackathon/*.test.ts
	@cd $(HARNESS) && node benchmarks/hackathon/run.ts --agent reference --suite all
	@test ! -x $(BIN) || (cd $(HARNESS) && node benchmarks/hackathon/run.ts --agent forge-cheat --bin $(BIN) --suite all)
	@$(HARNESS)/harness/check-layout.sh

clean:
	@cd $(HARNESS) && cargo clean
	@rm -rf $(HARNESS)/evidence $(HARNESS)/benchmarks/reports/hackathon
	@echo "clean: build output and evidence removed (node_modules kept; rm -rf node_modules to drop it)"
