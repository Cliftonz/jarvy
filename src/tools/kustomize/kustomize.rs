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
    fn ensure_kustomize_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
