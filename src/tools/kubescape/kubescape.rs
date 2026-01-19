//! kubescape - Kubernetes security scanner
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(KUBESCAPE, {
    command: "kubescape",
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
    fn ensure_kubescape_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
