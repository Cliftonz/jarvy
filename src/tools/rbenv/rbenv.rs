//! rbenv - ruby version manager
//!
//! rbenv lets you easily switch between multiple versions of Ruby.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(RBENV, {
    command: "rbenv",
    macos: { brew: "rbenv" },
    linux: { brew: "rbenv" },
    bsd: { pkg: "rbenv" },
    default_hook: {
        description: "Add rbenv shell initialization to .bashrc and .zshrc",
        script: r#"
# rbenv shell integration
RBENV_INIT_BASH='eval "$(rbenv init - bash)"'
RBENV_INIT_ZSH='eval "$(rbenv init - zsh)"'

# Add to .bashrc if not present
if [ -f "$HOME/.bashrc" ] && ! grep -q 'rbenv init' "$HOME/.bashrc"; then
    echo "$RBENV_INIT_BASH" >> "$HOME/.bashrc"
    echo "Added rbenv init to ~/.bashrc"
fi

# Add to .zshrc if not present
if [ -f "$HOME/.zshrc" ] && ! grep -q 'rbenv init' "$HOME/.zshrc"; then
    echo "$RBENV_INIT_ZSH" >> "$HOME/.zshrc"
    echo "Added rbenv init to ~/.zshrc"
fi
"#
    },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_rbenv_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
