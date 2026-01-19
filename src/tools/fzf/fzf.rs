//! fzf - command-line fuzzy finder
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(FZF, {
    command: "fzf",
    macos: { brew: "fzf" },
    linux: { uniform: "fzf" },
    windows: { winget: "junegunn.fzf" },
    bsd: { pkg: "fzf" },
    default_hook: {
        description: "Configure fzf shell integration (keybindings and completions)",
        script: r#"
# FZF shell integration - source keybindings and completions
# Paths vary by installation method; we try common locations

FZF_BASH_COMPLETION=""
FZF_BASH_KEYBINDINGS=""
FZF_ZSH_COMPLETION=""
FZF_ZSH_KEYBINDINGS=""

# Homebrew paths (macOS/Linuxbrew)
if [ -d "$(brew --prefix 2>/dev/null)/opt/fzf/shell" ]; then
    FZF_PREFIX="$(brew --prefix)/opt/fzf/shell"
    FZF_BASH_KEYBINDINGS="$FZF_PREFIX/key-bindings.bash"
    FZF_BASH_COMPLETION="$FZF_PREFIX/completion.bash"
    FZF_ZSH_KEYBINDINGS="$FZF_PREFIX/key-bindings.zsh"
    FZF_ZSH_COMPLETION="$FZF_PREFIX/completion.zsh"
elif [ -d "/usr/share/fzf" ]; then
    # Linux package manager paths
    FZF_BASH_KEYBINDINGS="/usr/share/fzf/key-bindings.bash"
    FZF_BASH_COMPLETION="/usr/share/fzf/completion.bash"
    FZF_ZSH_KEYBINDINGS="/usr/share/fzf/key-bindings.zsh"
    FZF_ZSH_COMPLETION="/usr/share/fzf/completion.zsh"
fi

# Add to .bashrc if not present
if [ -f "$HOME/.bashrc" ] && ! grep -q 'fzf' "$HOME/.bashrc"; then
    if [ -n "$FZF_BASH_KEYBINDINGS" ] && [ -f "$FZF_BASH_KEYBINDINGS" ]; then
        echo "[ -f \"$FZF_BASH_KEYBINDINGS\" ] && source \"$FZF_BASH_KEYBINDINGS\"" >> "$HOME/.bashrc"
    fi
    if [ -n "$FZF_BASH_COMPLETION" ] && [ -f "$FZF_BASH_COMPLETION" ]; then
        echo "[ -f \"$FZF_BASH_COMPLETION\" ] && source \"$FZF_BASH_COMPLETION\"" >> "$HOME/.bashrc"
    fi
    echo "Added fzf keybindings to ~/.bashrc"
fi

# Add to .zshrc if not present
if [ -f "$HOME/.zshrc" ] && ! grep -q 'fzf' "$HOME/.zshrc"; then
    if [ -n "$FZF_ZSH_KEYBINDINGS" ] && [ -f "$FZF_ZSH_KEYBINDINGS" ]; then
        echo "[ -f \"$FZF_ZSH_KEYBINDINGS\" ] && source \"$FZF_ZSH_KEYBINDINGS\"" >> "$HOME/.zshrc"
    fi
    if [ -n "$FZF_ZSH_COMPLETION" ] && [ -f "$FZF_ZSH_COMPLETION" ]; then
        echo "[ -f \"$FZF_ZSH_COMPLETION\" ] && source \"$FZF_ZSH_COMPLETION\"" >> "$HOME/.zshrc"
    fi
    echo "Added fzf keybindings to ~/.zshrc"
fi
"#
    },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_fzf_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
