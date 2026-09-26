#!/usr/bin/env zsh

# Enable prompt substitution for RPROMPT
setopt PROMPT_SUBST

# Model and agent info with token count
# Fully formatted output directly from Rust
# Returns ZSH-formatted string ready for use in RPROMPT
function _peach_prompt_info() {
    local peach_bin="${_PEACH_BIN:-${PEACH_BIN:-peach}}"
    
    # Get fully formatted prompt from peach (single command).
    # Pass session model/provider as CLI flags when set so the rprompt
    # reflects the active session override rather than global config.
    local -a peach_cmd
    peach_cmd=("$peach_bin")
    peach_cmd+=(zsh rprompt)
    [[ -n "$_PEACH_SESSION_MODEL" ]] && local -x PEACH_SESSION__MODEL_ID="$_PEACH_SESSION_MODEL"
    [[ -n "$_PEACH_SESSION_PROVIDER" ]] && local -x PEACH_SESSION__PROVIDER_ID="$_PEACH_SESSION_PROVIDER"
    [[ -n "$_PEACH_SESSION_REASONING_EFFORT" ]] && local -x PEACH_REASONING__EFFORT="$_PEACH_SESSION_REASONING_EFFORT"
    _PEACH_CONVERSATION_ID=$_PEACH_CONVERSATION_ID _PEACH_ACTIVE_AGENT=$_PEACH_ACTIVE_AGENT COLUMNS=$COLUMNS "${peach_cmd[@]}" 2>/dev/null
}

# Right prompt: agent and model with token count (uses single peach prompt command)
# Set RPROMPT if empty, otherwise append to existing value
if [[ -z "$_PEACH_THEME_LOADED" ]]; then
    RPROMPT='$(_peach_prompt_info)'"${RPROMPT:+ ${RPROMPT}}"
fi
