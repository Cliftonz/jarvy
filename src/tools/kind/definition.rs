//! kind - run local Kubernetes clusters using Docker
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(KIND, {
    command: "kind",
    repo: "kubernetes-sigs/kind",
    macos: { brew: "kind" },
    linux: { brew: "kind" },
    windows: { winget: "Kubernetes.kind" },
    bsd: { pkg: "kind" },
    default_hook: {
        description: "Install kind shell completions for bash and zsh",
        script: r#"
# Generate and install kind completions for bash
if [ -f "$HOME/.bashrc" ]; then
    mkdir -p "$HOME/.local/share/bash-completion/completions"
    kind completion bash > "$HOME/.local/share/bash-completion/completions/kind" 2>/dev/null || true
fi

# Generate and install kind completions for zsh
if [ -f "$HOME/.zshrc" ]; then
    mkdir -p "$HOME/.zsh/completions"
    kind completion zsh > "$HOME/.zsh/completions/_kind" 2>/dev/null || true
    if ! grep -q 'fpath.*\.zsh/completions' "$HOME/.zshrc"; then
        echo 'fpath=($HOME/.zsh/completions $fpath)' >> "$HOME/.zshrc"
    fi
fi
"#
    },
    // Kubernetes-in-Docker requires Docker runtime
    depends_on: &["docker"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_registration_shape() {
        assert_eq!(KIND.command, "kind");
        let mac = KIND.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("kind"));
        let win = KIND.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Kubernetes.kind"));
    }
}
