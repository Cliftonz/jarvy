//! kubens - kubectl context/namespace switcher
//!
//! kubens is a tool to switch between Kubernetes namespaces easily.
//! Part of the kubectx project.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(KUBENS, {
    command: "kubens",
    macos: { brew: "kubectx" },
    linux: { brew: "kubectx" },
    bsd: { pkg: "kubectx" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_kubens_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
