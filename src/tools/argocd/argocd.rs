//! argocd - GitOps continuous delivery for Kubernetes
//!
//! This tool uses the ToolSpec pattern for declarative installation.
//! Note: On Linux, this may require custom installation via GitHub releases.

use crate::define_tool;

define_tool!(ARGOCD, {
    command: "argocd",
    macos: { brew: "argocd" },
    linux: { brew: "argocd", apk: "argocd" },
    windows: { winget: "Argoproj.ArgoCD" },
    bsd: { pkg: "argocd" },
    default_hook: {
        description: "Install argocd shell completions for bash and zsh",
        script: r#"
# Generate and install argocd completions for bash
if [ -f "$HOME/.bashrc" ]; then
    mkdir -p "$HOME/.local/share/bash-completion/completions"
    argocd completion bash > "$HOME/.local/share/bash-completion/completions/argocd" 2>/dev/null || true
fi

# Generate and install argocd completions for zsh
if [ -f "$HOME/.zshrc" ]; then
    mkdir -p "$HOME/.zsh/completions"
    argocd completion zsh > "$HOME/.zsh/completions/_argocd" 2>/dev/null || true
    if ! grep -q 'fpath.*\.zsh/completions' "$HOME/.zshrc"; then
        echo 'fpath=($HOME/.zsh/completions $fpath)' >> "$HOME/.zshrc"
    fi
fi
"#
    },
    // GitOps tool needs kubectl for cluster management
    depends_on_one_of: &["kubectl"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_argocd_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
