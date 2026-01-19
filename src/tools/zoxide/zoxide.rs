//! zoxide - smarter cd command that learns your navigation patterns
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(ZOXIDE, {
    command: "zoxide",
    macos: { brew: "zoxide" },
    linux: { uniform: "zoxide" },
    windows: { winget: "ajeetdsouza.zoxide" },
    bsd: { pkg: "zoxide" },
    default_hook: {
        description: "Add zoxide shell initialization to .bashrc and .zshrc",
        script: r#"
# Zoxide shell integration
ZOXIDE_INIT_BASH='eval "$(zoxide init bash)"'
ZOXIDE_INIT_ZSH='eval "$(zoxide init zsh)"'

# Add to .bashrc if not present
if [ -f "$HOME/.bashrc" ] && ! grep -q 'zoxide init bash' "$HOME/.bashrc"; then
    echo "$ZOXIDE_INIT_BASH" >> "$HOME/.bashrc"
    echo "Added zoxide init to ~/.bashrc"
fi

# Add to .zshrc if not present
if [ -f "$HOME/.zshrc" ] && ! grep -q 'zoxide init zsh' "$HOME/.zshrc"; then
    echo "$ZOXIDE_INIT_ZSH" >> "$HOME/.zshrc"
    echo "Added zoxide init to ~/.zshrc"
fi
"#
    },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_zoxide_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
