//! atuin - magical shell history
//!
//! Atuin replaces your existing shell history with a SQLite database,
//! and records additional context for your commands.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(ATUIN, {
    command: "atuin",
    macos: { brew: "atuin" },
    linux: { uniform: "atuin" },
    windows: { winget: "atuinsh.atuin" },
    bsd: { pkg: "atuin" },
    default_hook: {
        description: "Add atuin shell initialization to .bashrc and .zshrc",
        script: r#"
# Atuin shell integration
ATUIN_INIT_BASH='eval "$(atuin init bash)"'
ATUIN_INIT_ZSH='eval "$(atuin init zsh)"'

# Add to .bashrc if not present
if [ -f "$HOME/.bashrc" ] && ! grep -q 'atuin init bash' "$HOME/.bashrc"; then
    echo "$ATUIN_INIT_BASH" >> "$HOME/.bashrc"
    echo "Added atuin init to ~/.bashrc"
fi

# Add to .zshrc if not present
if [ -f "$HOME/.zshrc" ] && ! grep -q 'atuin init zsh' "$HOME/.zshrc"; then
    echo "$ATUIN_INIT_ZSH" >> "$HOME/.zshrc"
    echo "Added atuin init to ~/.zshrc"
fi
"#
    },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_atuin_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
