#!/usr/bin/env zsh

# Configuration variables for peach plugin
# Using typeset to keep variables local to plugin scope and prevent public exposure

typeset -h _PEACH_BIN="${PEACH_BIN:-peach}"
typeset -h _PEACH_CONVERSATION_PATTERN=":"
typeset -h _PEACH_MAX_COMMIT_DIFF="${PEACH_MAX_COMMIT_DIFF:-100000}"

typeset -h _PEACH_COMMANDS=""

# Hidden variables to be used only via the PeachCLI
typeset -h _PEACH_CONVERSATION_ID
typeset -h _PEACH_ACTIVE_AGENT

# Previous conversation ID for :conversation - (like cd -)
typeset -h _PEACH_PREVIOUS_CONVERSATION_ID

# Session-scoped model and provider overrides (set via :model / :m).
# When non-empty, these are passed as --model / --provider to every peach
# invocation for the lifetime of the current shell session.
typeset -h _PEACH_SESSION_MODEL
typeset -h _PEACH_SESSION_PROVIDER

# Session-scoped reasoning effort override (set via :reasoning-effort / :re).
# When non-empty, exported as PEACH_REASONING__EFFORT for every peach invocation.
typeset -h _PEACH_SESSION_REASONING_EFFORT

# Terminal context capture settings
# Master switch for terminal context capture (preexec/precmd hooks)
typeset -h _PEACH_TERM="${PEACH_TERM:-true}"
# Maximum number of commands to keep in the ring buffer (metadata: cmd + exit code)
typeset -h _PEACH_TERM_MAX_COMMANDS="${PEACH_TERM_MAX_COMMANDS:-5}"
# OSC 133 semantic prompt marker emission: "auto", "on", or "off"
typeset -h _PEACH_TERM_OSC133="${PEACH_TERM_OSC133:-auto}"
# Ring buffer arrays for context capture
typeset -ha _PEACH_TERM_COMMANDS=()
typeset -ha _PEACH_TERM_EXIT_CODES=()
typeset -ha _PEACH_TERM_TIMESTAMPS=()
