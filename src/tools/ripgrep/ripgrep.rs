//! ripgrep - fast regex search tool
//!
//! This tool uses the ToolSpec pattern for declarative installation.
//! Note: The command is "rg" but the package name is "ripgrep".

use crate::define_tool;

define_tool!(RIPGREP, {
    command: "rg",
    macos: { brew: "ripgrep" },
    linux: { uniform: "ripgrep" },
    windows: { winget: "BurntSushi.ripgrep.MSVC" },
    bsd: { pkg: "ripgrep" },
    default_hook: {
        description: "Configure ripgrep shell completion",
        script: r#"
# Generate and add ripgrep completion to .bashrc
if [ -f "$HOME/.bashrc" ]; then
    mkdir -p "$HOME/.local/share/bash-completion/completions"
    rg --generate=complete-bash > "$HOME/.local/share/bash-completion/completions/rg" 2>/dev/null || true
fi

# Generate and add ripgrep completion to .zshrc
if [ -f "$HOME/.zshrc" ]; then
    mkdir -p "$HOME/.zsh/completions"
    rg --generate=complete-zsh > "$HOME/.zsh/completions/_rg" 2>/dev/null || true
    if ! grep -q 'fpath.*\.zsh/completions' "$HOME/.zshrc"; then
        echo 'fpath=($HOME/.zsh/completions $fpath)' >> "$HOME/.zshrc"
    fi
fi
"#
    },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_ripgrep_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
