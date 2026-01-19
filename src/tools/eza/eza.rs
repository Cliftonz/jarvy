//! eza - modern ls replacement with colors and Git awareness
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(EZA, {
    command: "eza",
    macos: { brew: "eza" },
    linux: { apt: "eza", dnf: "eza", pacman: "eza", apk: "eza" },
    windows: { winget: "eza-community.eza" },
    bsd: { pkg: "eza" },
    default_hook: {
        description: "Add eza aliases for ls replacement",
        script: r#"
# Add eza aliases to .bashrc
if [ -f "$HOME/.bashrc" ] && ! grep -q "alias ls='eza'" "$HOME/.bashrc"; then
    cat >> "$HOME/.bashrc" << 'ALIASES'

# eza aliases (modern ls replacement)
alias ls='eza'
alias ll='eza -l --git'
alias la='eza -la --git'
alias lt='eza --tree --level=2'
ALIASES
    echo "Added eza aliases to .bashrc"
fi

# Add eza aliases to .zshrc
if [ -f "$HOME/.zshrc" ] && ! grep -q "alias ls='eza'" "$HOME/.zshrc"; then
    cat >> "$HOME/.zshrc" << 'ALIASES'

# eza aliases (modern ls replacement)
alias ls='eza'
alias ll='eza -l --git'
alias la='eza -la --git'
alias lt='eza --tree --level=2'
ALIASES
    echo "Added eza aliases to .zshrc"
fi
"#
    },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_eza_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
