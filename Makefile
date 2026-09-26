# Peach Ice Tea — evaluator interface (docs/harness/MAKEFILE_EVAL.md, D-070).
#
#   export AI_API_KEY=...        # the only credential; never written anywhere
#   make setup                   # build the harness and install what it needs
#   make run REPO=/path/to/repo  # then type or pipe the issue (end with Ctrl-D)
#   make run REPO=... PROMPT="Fix ..."   # or give it inline
#   make test                    # the local hackathon suite, live (spends model requests)
#   make check                   # offline checks, no key and no model needed
#   make ui                      # local web UI: run tasks, watch them live, browse evidence (D-092)
#   make clean                   # remove build output and evidence
#
# Variables: PROFILE (default: chosen from AI_API_KEY's shape, else gemini; D-081), MODEL (override the
# profile's model, e.g. the one the organisers prescribe), REPO, PROMPT, TEST_COMMAND,
# EVIDENCE_DIR, MAX_DURATION_SECS. AI_API_KEY is handed to the profile as the provider
# variable it expects, so changing provider never means changing source.

SHELL := /bin/bash
.SHELLFLAGS := -eu -o pipefail -c
.DEFAULT_GOAL := help

HARNESS := $(patsubst %/,%,$(dir $(abspath $(lastword $(MAKEFILE_LIST)))))
PROFILE ?=
BIN := $(HARNESS)/target/release/forge
PROTOC_VERSION := 36.2
PROTOC_TOOLS_DIR := $(HARNESS)/.tools/protoc
# Where make was invoked from: `make -f <harness>/Makefile run` inside the target repo works.
REPO ?= $(CURDIR)

export PROFILE MODEL PROMPT REPO TEST_COMMAND EVIDENCE_DIR MAX_DURATION_SECS

.PHONY: help setup run test check clean ui

help:
	@sed -n '2,16p' $(HARNESS)/Makefile | sed 's/^# \{0,1\}//'

setup:
	@command -v cargo >/dev/null || { \
	  echo "setup: installing the Rust toolchain (rustup)"; \
	  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path; }
	@PATH="$(PROTOC_TOOLS_DIR)/bin:$$PATH" command -v protoc >/dev/null || { \
	  if command -v brew >/dev/null; then echo "setup: installing protoc (brew)"; brew install protobuf; \
	  elif [ "$$(uname -s)" = "Linux" ]; then \
	    echo "setup: protoc not found; trying apt-get non-interactively"; \
	    HAVE_APT=0; SUDO=""; \
	    if command -v apt-get >/dev/null; then \
	      if [ "$$(id -u)" = "0" ]; then HAVE_APT=1; \
	      elif command -v sudo >/dev/null && sudo -n true 2>/dev/null; then SUDO="sudo -n"; HAVE_APT=1; \
	      else echo "setup: no root and no passwordless sudo; skipping apt-get" >&2; fi; \
	    fi; \
	    APT_OK=0; \
	    if [ "$$HAVE_APT" = "1" ]; then \
	      DEBIAN_FRONTEND=noninteractive $$SUDO apt-get update -qq \
	        && DEBIAN_FRONTEND=noninteractive $$SUDO apt-get install -y -qq --no-install-recommends protobuf-compiler \
	        && APT_OK=1; \
	    fi; \
	    if [ "$$APT_OK" != "1" ] || ! command -v protoc >/dev/null; then \
	      echo "setup: apt-get unavailable/failed; falling back to a pinned protoc $(PROTOC_VERSION) release binary"; \
	      command -v unzip >/dev/null || { echo "setup: 'unzip' is required to unpack the protoc release (and apt-get isn't usable); install protoc manually (see docs/harness/DEV.md)" >&2; exit 1; }; \
	      ARCH="$$(uname -m)"; \
	      case "$$ARCH" in \
	        x86_64|amd64) PB_ARCH=x86_64 ;; \
	        aarch64|arm64) PB_ARCH=aarch_64 ;; \
	        i686|i386) PB_ARCH=x86_32 ;; \
	        ppc64le) PB_ARCH=ppcle_64 ;; \
	        s390x) PB_ARCH=s390_64 ;; \
	        *) echo "setup: unsupported architecture '$$ARCH' for protoc download; install protoc manually (see docs/harness/DEV.md)" >&2; exit 1 ;; \
	      esac; \
	      PB_URL="https://github.com/protocolbuffers/protobuf/releases/download/v$(PROTOC_VERSION)/protoc-$(PROTOC_VERSION)-linux-$${PB_ARCH}.zip"; \
	      mkdir -p $(PROTOC_TOOLS_DIR); \
	      TMP_ZIP="$$(mktemp)"; \
	      curl --proto '=https' --tlsv1.2 -sSfL "$$PB_URL" -o "$$TMP_ZIP" \
	        || { echo "setup: failed to download $$PB_URL (no network access to GitHub releases?)" >&2; rm -f "$$TMP_ZIP"; exit 1; }; \
	      unzip -q -o "$$TMP_ZIP" -d $(PROTOC_TOOLS_DIR) bin/protoc 'include/*' \
	        || { echo "setup: failed to unpack the protoc archive" >&2; rm -f "$$TMP_ZIP"; exit 1; }; \
	      rm -f "$$TMP_ZIP"; chmod +x $(PROTOC_TOOLS_DIR)/bin/protoc; \
	      echo "setup: protoc $(PROTOC_VERSION) installed to $(PROTOC_TOOLS_DIR)/bin (used via PATH for this build)"; \
	    fi; \
	  else echo "setup: protoc is required: apt-get install -y protobuf-compiler (or see docs/harness/DEV.md)" >&2; exit 1; \
	  fi; }
	@command -v node >/dev/null || { echo "setup: Node.js >= 22 is required for make test (https://nodejs.org)" >&2; exit 1; }
	@PATH="$(PROTOC_TOOLS_DIR)/bin:$$HOME/.cargo/bin:$$PATH" $(HARNESS)/harness/build.sh
	@cd $(HARNESS) && npm ci --no-audit --no-fund --loglevel=error
	@$(HARNESS)/harness/check-layout.sh
	@echo "setup: done. Next: export AI_API_KEY=... && make run REPO=/path/to/repository"

run:
	@test -x $(BIN) || { echo "run: no harness binary; run make setup first" >&2; exit 1; }
	@PEACH_ICE_TEA_BIN=$(BIN) $(HARNESS)/harness/run-task

test:
	@test -x $(BIN) || { echo "test: no harness binary; run make setup first" >&2; exit 1; }
	@test -n "$${AI_API_KEY:-}" || { echo "test: AI_API_KEY is not set (make check needs no key)" >&2; exit 1; }
	@PROFILE=$$($(HARNESS)/harness/select-profile); \
	  KEY_VAR=$$(sed -n 's/^PROFILE_KEY_VAR=//p' $(HARNESS)/configuration/profiles/$$PROFILE/profile.env); \
	  export "$$KEY_VAR=$$AI_API_KEY"; unset AI_API_KEY; \
	  if [ -n "$${MODEL:-}" ]; then export FORGE_SESSION__MODEL_ID="$$MODEL"; fi; \
	  cd $(HARNESS) && node benchmarks/hackathon/run.ts --agent forge --profile $$PROFILE --bin $(BIN) --suite all

check:
	@cd $(HARNESS) && npx tsx --test benchmarks/hackathon/*.test.ts harness/ui/*.test.ts
	@cd $(HARNESS) && node benchmarks/hackathon/run.ts --agent reference --suite all
	@test ! -x $(BIN) || (cd $(HARNESS) && node benchmarks/hackathon/run.ts --agent forge-cheat --bin $(BIN) --suite all)
	@$(HARNESS)/harness/check-layout.sh

# Binds 127.0.0.1 only. Keys come from this shell's environment (export AI_API_KEY or a
# profile's own variable first); the page only learns which profiles have one.
ui:
	@test -x $(BIN) || test -x $(HARNESS)/target/debug/forge || { echo "ui: no harness binary; run make setup first" >&2; exit 1; }
	@UI_DEFAULT_REPO="$(if $(filter $(HARNESS),$(abspath $(REPO))),,$(abspath $(REPO)))" node $(HARNESS)/harness/ui/server.ts

clean:
	@cd $(HARNESS) && cargo clean
	@rm -rf $(HARNESS)/evidence $(HARNESS)/benchmarks/reports/hackathon
	@echo "clean: build output and evidence removed (node_modules kept; rm -rf node_modules to drop it)"
