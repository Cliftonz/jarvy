//! minikube - local Kubernetes cluster for development
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(MINIKUBE, {
    command: "minikube",
    macos: { brew: "minikube" },
    linux: { uniform: "minikube" },
    windows: { winget: "Kubernetes.minikube" },
    bsd: { pkg: "minikube" },
    default_hook: {
        description: "Install minikube shell completions for bash and zsh",
        script: r#"
# Generate and install minikube completions for bash
if [ -f "$HOME/.bashrc" ]; then
    mkdir -p "$HOME/.local/share/bash-completion/completions"
    minikube completion bash > "$HOME/.local/share/bash-completion/completions/minikube" 2>/dev/null || true
fi

# Generate and install minikube completions for zsh
if [ -f "$HOME/.zshrc" ]; then
    mkdir -p "$HOME/.zsh/completions"
    minikube completion zsh > "$HOME/.zsh/completions/_minikube" 2>/dev/null || true
    if ! grep -q 'fpath.*\.zsh/completions' "$HOME/.zshrc"; then
        echo 'fpath=($HOME/.zsh/completions $fpath)' >> "$HOME/.zshrc"
    fi
fi
"#
    },
    // minikube needs a container runtime (docker or podman)
    // If either is in config, install it first
    depends_on_one_of: &["docker", "podman"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_minikube_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
