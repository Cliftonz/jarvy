//! eksctl - AWS EKS cluster management CLI
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(EKSCTL, {
    command: "eksctl",
    macos: { brew: "eksctl" },
    linux: { uniform: "eksctl" },
    windows: { winget: "weaveworks.eksctl" },
    bsd: { pkg: "eksctl" },
    default_hook: {
        description: "Install eksctl shell completions for bash and zsh",
        script: r#"
# Generate and install eksctl completions for bash
if [ -f "$HOME/.bashrc" ]; then
    mkdir -p "$HOME/.local/share/bash-completion/completions"
    eksctl completion bash > "$HOME/.local/share/bash-completion/completions/eksctl" 2>/dev/null || true
fi

# Generate and install eksctl completions for zsh
if [ -f "$HOME/.zshrc" ]; then
    mkdir -p "$HOME/.zsh/completions"
    eksctl completion zsh > "$HOME/.zsh/completions/_eksctl" 2>/dev/null || true
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
    fn ensure_eksctl_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
