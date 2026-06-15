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
    fn kubectx_registration_shape() {
        assert_eq!(KUBECTX.command, "kubectx");
        let mac = KUBECTX.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("kubectx"));
        let win = KUBECTX.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("ahmetb.kubectx"));
    }
}
