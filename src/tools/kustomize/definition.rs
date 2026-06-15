//! kustomize - Kubernetes YAML customization without templates
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(KUSTOMIZE, {
    command: "kustomize",
    macos: { brew: "kustomize" },
    linux: { uniform: "kustomize" },
    windows: { winget: "Kubernetes.kustomize" },
    bsd: { pkg: "kustomize" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kustomize_registration_shape() {
        assert_eq!(KUSTOMIZE.command, "kustomize");
        let mac = KUSTOMIZE.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("kustomize"));
        let win = KUSTOMIZE.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Kubernetes.kustomize"));
    }
}
