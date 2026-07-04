//! helm - package manager for Kubernetes applications
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(HELM, {
    command: "helm",
    repo: "helm/helm",
    macos: { brew: "helm" },
    linux: { uniform: "helm" },
    windows: { winget: "Helm.Helm" },
    bsd: { pkg: "helm" },
    default_hook: {
        description: "Add common Helm chart repositories",
        script: r#"
# Add bitnami repository if not present
if ! helm repo list 2>/dev/null | grep -q 'bitnami'; then
    helm repo add bitnami https://charts.bitnami.com/bitnami 2>/dev/null && \
        echo "Added bitnami Helm repository" || true
fi

# Update repositories
helm repo update 2>/dev/null || true
"#
    },
    // Helm needs kubectl/kubeconfig to deploy to a cluster
    depends_on_one_of: &["kubectl"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helm_registration_shape() {
        assert_eq!(HELM.command, "helm");
        let mac = HELM.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("helm"));
        let win = HELM.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Helm.Helm"));
    }
}
