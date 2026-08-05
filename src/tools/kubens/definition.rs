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
    // No first-party winget manifest; the go fallback route covers
    // Windows (verified 2026-08).
    fallback: { go: "github.com/ahmetb/kubectx/cmd/kubens" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kubens_registration_shape() {
        assert_eq!(KUBENS.command, "kubens");
        let mac = KUBENS.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("kubectx"));
        assert!(KUBENS.windows.is_none(), "no first-party winget manifest");
        assert_eq!(KUBENS.fallback.len(), 1);
        assert_eq!(KUBENS.fallback[0].eco, crate::tools::spec::FallbackEco::Go);
        assert_eq!(
            KUBENS.fallback[0].package,
            "github.com/ahmetb/kubectx/cmd/kubens"
        );
    }
}
