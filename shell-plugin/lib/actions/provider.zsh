#!/usr/bin/env zsh

# Provider selection action handlers

# Action handler: Select the provider for the current session.
# Sets _PEACH_SESSION_PROVIDER in the shell environment so that every
# subsequent peach invocation uses that provider via --provider flag
# without touching the permanent global configuration.
function _peach_action_session_provider() {
    local input_text="$1"
    echo

    local selected
    selected=$(_peach_select_with_query "$input_text" provider)

    if [[ -n "$selected" ]]; then
        _PEACH_SESSION_PROVIDER="$selected"
        _peach_log success "Session provider set to \033[1m${selected}\033[0m"
    fi
}
