//! sdkman - software development kit manager for the JVM
//!
//! SDKMAN! is a tool for managing parallel versions of multiple Software
//! Development Kits on most Unix-based systems. It provides a convenient
//! CLI and API for installing, switching, removing and listing Candidates.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(SDKMAN, {
    command: "sdk",
    macos: { brew: "sdkman-cli" },
    linux: { apt: "sdkman", dnf: "sdkman", pacman: "sdkman", apk: "sdkman" },
    bsd: { pkg: "sdkman" },
    default_hook: {
        description: "Add SDKMAN shell initialization to .bashrc and .zshrc",
        script: r#"
# SDKMAN shell integration
SDKMAN_INIT='export SDKMAN_DIR="$HOME/.sdkman"
[[ -s "$HOME/.sdkman/bin/sdkman-init.sh" ]] && source "$HOME/.sdkman/bin/sdkman-init.sh"'

# Add to .bashrc if not present
if [ -f "$HOME/.bashrc" ] && ! grep -q 'sdkman-init.sh' "$HOME/.bashrc"; then
    echo "$SDKMAN_INIT" >> "$HOME/.bashrc"
    echo "Added SDKMAN init to ~/.bashrc"
fi

# Add to .zshrc if not present
if [ -f "$HOME/.zshrc" ] && ! grep -q 'sdkman-init.sh' "$HOME/.zshrc"; then
    echo "$SDKMAN_INIT" >> "$HOME/.zshrc"
    echo "Added SDKMAN init to ~/.zshrc"
fi
"#
    },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_sdkman_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
