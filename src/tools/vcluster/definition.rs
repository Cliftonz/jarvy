//! vcluster — virtual Kubernetes clusters that run inside a namespace of
//! a host cluster, giving teams isolated multi-tenancy without the cost
//! of separate control planes.
//!
//! Deploys the virtual control plane via helm and talks to the host
//! cluster through kubeconfig, so depends on `kubectl` and `helm`
//! (mirrors the upstream brew formula's dependency list).

use crate::define_tool;

define_tool!(VCLUSTER, {
    command: "vcluster",
    macos: { brew: "vcluster" },
    linux: { brew: "vcluster" },
    // No first-party winget manifest as of 2026-08; upstream ships
    // release binaries at https://github.com/loft-sh/vcluster/releases.
    depends_on: &["kubectl", "helm"],
    category: "devops",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vcluster_registration_shape() {
        assert_eq!(VCLUSTER.command, "vcluster");
        assert_eq!(VCLUSTER.macos.expect("macOS").brew, Some("vcluster"));
        assert_eq!(VCLUSTER.linux.expect("Linux").brew, Some("vcluster"));
        assert!(VCLUSTER.windows.is_none(), "no first-party winget yet");
        let deps = VCLUSTER.depends_on.expect("declares deps");
        assert!(deps.contains(&"kubectl"));
        assert!(deps.contains(&"helm"));
    }
}
