//! kubescape - Kubernetes security scanner
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(KUBESCAPE, {
    command: "kubescape",
    repo: "kubescape/kubescape",
    macos: { brew: "kubescape" },
    linux: { uniform: "kubescape" },
    windows: { winget: "kubescape.kubescape" },
    bsd: { pkg: "kubescape" },
    depends_on: &["kubectl"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kubescape_registration_shape() {
        assert_eq!(KUBESCAPE.command, "kubescape");
        let mac = KUBESCAPE.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("kubescape"));
        let win = KUBESCAPE.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("kubescape.kubescape"));
    }
}
