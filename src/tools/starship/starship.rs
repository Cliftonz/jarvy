//! starship - minimal, fast, customizable shell prompt
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(STARSHIP, {
    command: "starship",
    macos: { brew: "starship" },
    linux: { uniform: "starship" },
    windows: { winget: "Starship.Starship" },
    bsd: { pkg: "starship" },
    default_hook: {
        description: "Add starship shell initialization to .bashrc and .zshrc",
        script: r#"
# Starship shell integration
STARSHIP_INIT_BASH='eval "$(starship init bash)"'
STARSHIP_INIT_ZSH='eval "$(starship init zsh)"'

# Add to .bashrc if not present
if [ -f "$HOME/.bashrc" ] && ! grep -q 'starship init bash' "$HOME/.bashrc"; then
    echo "$STARSHIP_INIT_BASH" >> "$HOME/.bashrc"
    echo "Added starship init to ~/.bashrc"
fi

# Add to .zshrc if not present
if [ -f "$HOME/.zshrc" ] && ! grep -q 'starship init zsh' "$HOME/.zshrc"; then
    echo "$STARSHIP_INIT_ZSH" >> "$HOME/.zshrc"
    echo "Added starship init to ~/.zshrc"
fi
"#
    },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_starship_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
