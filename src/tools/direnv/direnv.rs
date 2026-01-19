//! direnv - directory-specific environment variables via .envrc
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(DIRENV, {
    command: "direnv",
    macos: { brew: "direnv" },
    linux: { uniform: "direnv" },
    windows: { winget: "direnv.direnv" },
    bsd: { pkg: "direnv" },
    default_hook: {
        description: "Add direnv shell hook to .bashrc and .zshrc",
        script: r#"
# Direnv shell hook
DIRENV_HOOK_BASH='eval "$(direnv hook bash)"'
DIRENV_HOOK_ZSH='eval "$(direnv hook zsh)"'

# Add to .bashrc if not present
if [ -f "$HOME/.bashrc" ] && ! grep -q 'direnv hook bash' "$HOME/.bashrc"; then
    echo "$DIRENV_HOOK_BASH" >> "$HOME/.bashrc"
    echo "Added direnv hook to ~/.bashrc"
fi

# Add to .zshrc if not present
if [ -f "$HOME/.zshrc" ] && ! grep -q 'direnv hook zsh' "$HOME/.zshrc"; then
    echo "$DIRENV_HOOK_ZSH" >> "$HOME/.zshrc"
    echo "Added direnv hook to ~/.zshrc"
fi
"#
    },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_direnv_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
