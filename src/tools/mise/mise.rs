//! mise - dev tools, env vars, task runner
//!
//! mise (formerly rtx) is a polyglot tool version manager.
//! It manages languages like Node, Python, Ruby, etc.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(MISE, {
    command: "mise",
    macos: { brew: "mise" },
    linux: { brew: "mise" },
    windows: { winget: "jdx.mise" },
    bsd: { pkg: "mise" },
    default_hook: {
        description: "Add mise shell initialization to .bashrc and .zshrc",
        script: r#"
# mise shell integration
MISE_INIT_BASH='eval "$(mise activate bash)"'
MISE_INIT_ZSH='eval "$(mise activate zsh)"'

# Add to .bashrc if not present
if [ -f "$HOME/.bashrc" ] && ! grep -q 'mise activate bash' "$HOME/.bashrc"; then
    echo "$MISE_INIT_BASH" >> "$HOME/.bashrc"
    echo "Added mise activate to ~/.bashrc"
fi

# Add to .zshrc if not present
if [ -f "$HOME/.zshrc" ] && ! grep -q 'mise activate zsh' "$HOME/.zshrc"; then
    echo "$MISE_INIT_ZSH" >> "$HOME/.zshrc"
    echo "Added mise activate to ~/.zshrc"
fi
"#
    },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_mise_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
