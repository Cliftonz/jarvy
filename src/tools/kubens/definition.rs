//! kubens - kubectl context/namespace switcher
//!
//! kubens is a tool to switch between Kubernetes namespaces easily.
//! Part of the kubectx project.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(KUBENS, {
    command: "kubens",
    repo: "ahmetb/kubectx",
    macos: { brew: "kubectx" },
    linux: { brew: "kubectx" },
    bsd: { pkg: "kubectx" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kubens_registration_shape() {
        assert_eq!(KUBENS.command, "kubens");
        let mac = KUBENS.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("kubectx"));
    }
}
