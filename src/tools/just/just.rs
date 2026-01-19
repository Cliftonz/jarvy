//! just - Command runner (make alternative)
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(JUST, {
    command: "just",
    macos: { brew: "just" },
    linux: { uniform: "just" },
    windows: { winget: "Casey.Just" },
    bsd: { pkg: "just" },
    default_hook: {
        description: "Install just shell completions for bash and zsh",
        script: r#"
# Generate and install just completions for bash
if [ -f "$HOME/.bashrc" ]; then
    mkdir -p "$HOME/.local/share/bash-completion/completions"
    just --completions bash > "$HOME/.local/share/bash-completion/completions/just" 2>/dev/null || true
fi

# Generate and install just completions for zsh
if [ -f "$HOME/.zshrc" ]; then
    mkdir -p "$HOME/.zsh/completions"
    just --completions zsh > "$HOME/.zsh/completions/_just" 2>/dev/null || true
    if ! grep -q 'fpath.*\.zsh/completions' "$HOME/.zshrc"; then
        echo 'fpath=($HOME/.zsh/completions $fpath)' >> "$HOME/.zshrc"
        echo "Added just completion path to .zshrc"
    fi
fi
"#
    },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_just_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
