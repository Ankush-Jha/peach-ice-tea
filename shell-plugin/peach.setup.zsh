# !! Contents within this block are managed by 'peach zsh setup' !!
# !! Do not edit manually - changes will be overwritten !!

# Add required zsh plugins if not already present
if [[ ! " ${plugins[@]} " =~ " zsh-autosuggestions " ]]; then
    plugins+=(zsh-autosuggestions)
fi
if [[ ! " ${plugins[@]} " =~ " zsh-syntax-highlighting " ]]; then
    plugins+=(zsh-syntax-highlighting)
fi

# Load peach shell plugin (commands, completions, keybindings) if not already loaded
if [[ -z "$_PEACH_PLUGIN_LOADED" ]]; then
    eval "$(peach zsh plugin)"
fi

# Load peach shell theme (prompt with AI context) if not already loaded
if [[ -z "$_PEACH_THEME_LOADED" ]]; then
    eval "$(peach zsh theme)"
fi
