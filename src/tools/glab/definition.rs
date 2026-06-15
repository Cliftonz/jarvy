//! glab - GitLab CLI
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(GLAB, {
    command: "glab",
    macos: { brew: "glab" },
    linux: { uniform: "glab" },
    windows: { winget: "GLab.GLab" },
    bsd: { pkg: "glab" },
    default_hook: {
        description: "Install glab shell completions for bash and zsh",
        script: r#"
# Generate and install glab completions for bash
if [ -f "$HOME/.bashrc" ]; then
    mkdir -p "$HOME/.local/share/bash-completion/completions"
    glab completion -s bash > "$HOME/.local/share/bash-completion/completions/glab" 2>/dev/null || true
fi

# Generate and install glab completions for zsh
if [ -f "$HOME/.zshrc" ]; then
    mkdir -p "$HOME/.zsh/completions"
    glab completion -s zsh > "$HOME/.zsh/completions/_glab" 2>/dev/null || true
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
    fn glab_registration_shape() {
        assert_eq!(GLAB.command, "glab");
        let mac = GLAB.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("glab"));
        let win = GLAB.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("GLab.GLab"));
    }
}
