//! k9s - terminal UI for Kubernetes cluster management
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(K9S, {
    command: "k9s",
    macos: { brew: "derailed/k9s/k9s" },
    linux: { uniform: "k9s" },
    windows: { winget: "Derailed.k9s" },
    bsd: { pkg: "k9s" },
    default_hook: {
        description: "Configure k9s shell completion",
        script: r#"
# Add k9s completion to .bashrc
if [ -f "$HOME/.bashrc" ] && ! grep -q 'k9s completion' "$HOME/.bashrc"; then
    echo 'source <(k9s completion bash)' >> "$HOME/.bashrc"
    echo "Added k9s completion to .bashrc"
fi

# Add k9s completion to .zshrc
if [ -f "$HOME/.zshrc" ] && ! grep -q 'k9s completion' "$HOME/.zshrc"; then
    echo 'source <(k9s completion zsh)' >> "$HOME/.zshrc"
    echo "Added k9s completion to .zshrc"
fi
"#
    },
    // K8s TUI needs kubectl to interact with clusters
    depends_on_one_of: &["kubectl"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_k9s_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
