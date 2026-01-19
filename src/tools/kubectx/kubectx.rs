//! kubectx - fast Kubernetes context and namespace switching
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(KUBECTX, {
    command: "kubectx",
    macos: { brew: "kubectx" },
    linux: { brew: "kubectx" },
    windows: { winget: "ahmetb.kubectx" },
    bsd: { pkg: "kubectx" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_kubectx_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
