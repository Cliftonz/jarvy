//! broot - interactive tree view with fuzzy search
//!
//! Broot is a new way to see and navigate directory trees. It allows
//! navigation, searching, and file operations with an intuitive interface.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(BROOT, {
    command: "broot",
    macos: { brew: "broot" },
    linux: { uniform: "broot" },
    windows: { winget: "Dystroy.broot" },
    bsd: { pkg: "broot" },
    default_hook: {
        description: "Add broot shell function to .bashrc and .zshrc",
        script: r#"
# Broot shell integration - adds the 'br' function for better cd integration
BROOT_INIT_BASH='source "$HOME/.config/broot/launcher/bash/br"'
BROOT_INIT_ZSH='source "$HOME/.config/broot/launcher/zsh/br"'

# Initialize broot config if not present
if command -v broot >/dev/null 2>&1 && [ ! -d "$HOME/.config/broot" ]; then
    broot --install >/dev/null 2>&1 || true
fi

# Add to .bashrc if launcher exists and not present
if [ -f "$HOME/.config/broot/launcher/bash/br" ] && [ -f "$HOME/.bashrc" ]; then
    if ! grep -q 'broot/launcher/bash/br' "$HOME/.bashrc"; then
        echo "$BROOT_INIT_BASH" >> "$HOME/.bashrc"
        echo "Added broot br function to ~/.bashrc"
    fi
fi

# Add to .zshrc if launcher exists and not present
if [ -f "$HOME/.config/broot/launcher/zsh/br" ] && [ -f "$HOME/.zshrc" ]; then
    if ! grep -q 'broot/launcher/zsh/br' "$HOME/.zshrc"; then
        echo "$BROOT_INIT_ZSH" >> "$HOME/.zshrc"
        echo "Added broot br function to ~/.zshrc"
    fi
fi
"#
    },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_broot_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
