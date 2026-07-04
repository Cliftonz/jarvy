//! kubectl - official Kubernetes command-line tool
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(KUBECTL, {
    command: "kubectl",
    repo: "kubernetes/kubernetes",
    macos: { brew: "kubectl" },
    linux: { uniform: "kubectl" },
    windows: { winget: "Kubernetes.kubectl" },
    bsd: { pkg: "kubectl" },
    default_hook: {
        description: "Enable kubectl shell completion and 'k' alias",
        script: r#"
# kubectl shell completion and alias
KUBECTL_BASH_COMPLETION='
# kubectl completion
source <(kubectl completion bash)
alias k=kubectl
complete -o default -F __start_kubectl k
'

KUBECTL_ZSH_COMPLETION='
# kubectl completion
source <(kubectl completion zsh)
alias k=kubectl
'

# Add to .bashrc if not present
if [ -f "$HOME/.bashrc" ] && ! grep -q 'kubectl completion bash' "$HOME/.bashrc"; then
    echo "$KUBECTL_BASH_COMPLETION" >> "$HOME/.bashrc"
    echo "Added kubectl completion to ~/.bashrc"
fi

# Add to .zshrc if not present
if [ -f "$HOME/.zshrc" ] && ! grep -q 'kubectl completion zsh' "$HOME/.zshrc"; then
    echo "$KUBECTL_ZSH_COMPLETION" >> "$HOME/.zshrc"
    echo "Added kubectl completion to ~/.zshrc"
fi
"#
    },
    // If any K8s cluster provider is in config, install it before kubectl
    // kubectl works with ANY of these - it just needs a cluster to talk to
    depends_on_one_of: &["minikube", "kind", "k3d", "docker", "podman"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kubectl_registration_shape() {
        assert_eq!(KUBECTL.command, "kubectl");
        let mac = KUBECTL.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("kubectl"));
        let win = KUBECTL.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Kubernetes.kubectl"));
    }
}
