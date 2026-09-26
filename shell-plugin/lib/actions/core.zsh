#!/usr/bin/env zsh

# Core action handlers for basic peach operations

# Action handler: Start a new conversation
function _peach_action_new() {
    local input_text="$1"
    
    # Clear conversation and save as previous (like cd -)
    _peach_clear_conversation
    _PEACH_ACTIVE_AGENT="peach"
    
    echo
    
    # If input_text is provided, send it to the new conversation
    if [[ -n "$input_text" ]]; then
        # Generate new conversation ID and switch to it
        local new_id=$($_PEACH_BIN conversation new)
        _peach_switch_conversation "$new_id"
        
        # Execute the peach command with the input text
        _peach_exec_interactive -p "$input_text" --cid "$_PEACH_CONVERSATION_ID"
        
        # Start background sync job if enabled and not already running
        _peach_start_background_sync
        # Start background update check
        _peach_start_background_update
    else
        # Only show banner if no input text (starting fresh conversation)
        _peach_exec banner
    fi
}

# Action handler: Show session info
function _peach_action_info() {
    echo
    if [[ -n "$_PEACH_CONVERSATION_ID" ]]; then
        _peach_exec info --cid "$_PEACH_CONVERSATION_ID"
    else
        _peach_exec info
    fi
}

# Action handler: Dump conversation
function _peach_action_dump() {
    local input_text="$1"
    if [[ "$input_text" == "html" ]]; then
        _peach_handle_conversation_command "dump" "--html"
    else
        _peach_handle_conversation_command "dump"
    fi
}

# Action handler: Compact conversation
function _peach_action_compact() {
    _peach_handle_conversation_command "compact"
}

# Action handler: Retry last message
function _peach_action_retry() {
    _peach_handle_conversation_command "retry"
}

# Action handler: Show available commands (mirrors :help in the REPL)
function _peach_action_help() {
    echo
    $_PEACH_BIN list command
}

# Helper function to handle conversation commands that require an active conversation
function _peach_handle_conversation_command() {
    local subcommand="$1"
    shift  # Remove first argument, remaining args become extra parameters
    
    echo
    
    # Check if PEACH_CONVERSATION_ID is set
    if [[ -z "$_PEACH_CONVERSATION_ID" ]]; then
        _peach_log error "No active conversation. Start a conversation first or use :conversation to see existing ones"
        return 0
    fi
    
    # Execute the conversation command with conversation ID and any extra arguments
    _peach_exec conversation "$subcommand" "$_PEACH_CONVERSATION_ID" "$@"
}
