//! lazygit - simple terminal UI for git commands
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(LAZYGIT, {
    command: "lazygit",
    macos: { brew: "lazygit" },
    linux: { uniform: "lazygit" },
    windows: { winget: "JesseDuffield.lazygit" },
    bsd: { pkg: "lazygit" },
    default_hook: {
        description: "Create lg alias for lazygit",
        script: r#"
# Add lg alias to .bashrc
if [ -f "$HOME/.bashrc" ] && ! grep -q 'alias lg=' "$HOME/.bashrc"; then
    echo 'alias lg=lazygit' >> "$HOME/.bashrc"
    echo "Added 'lg' alias for lazygit to .bashrc"
fi

# Add lg alias to .zshrc
if [ -f "$HOME/.zshrc" ] && ! grep -q 'alias lg=' "$HOME/.zshrc"; then
    echo 'alias lg=lazygit' >> "$HOME/.zshrc"
    echo "Added 'lg' alias for lazygit to .zshrc"
fi
"#
    },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_lazygit_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
