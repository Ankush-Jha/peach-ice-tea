#!/usr/bin/env zsh

# Authentication action handlers

# Action handler: Login to provider
function _peach_action_login() {
    local input_text="$1"
    echo

    local provider
    provider=$(_peach_select_with_query "$input_text" provider)

    if [[ -n "$provider" ]]; then
        _peach_exec_interactive provider login "$provider"
    fi
}

# Action handler: Logout from provider
function _peach_action_logout() {
    local input_text="$1"
    echo

    local provider
    provider=$(_peach_select_with_query "$input_text" provider --configured)

    if [[ -n "$provider" ]]; then
        _peach_exec provider logout "$provider"
    fi
}
