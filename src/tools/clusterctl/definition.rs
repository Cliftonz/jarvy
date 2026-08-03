//! clusterctl — Cluster API (CAPI) lifecycle management: initialize a
//! management cluster, generate workload cluster manifests, upgrade
//! providers.
//!
//! Platform-engineering focused; niche for general Kubernetes users but
//! essential for teams standing up clusters programmatically. Depends
//! on `kubectl` — every subcommand runs against a kubeconfig context.

use crate::define_tool;

define_tool!(CLUSTERCTL, {
    command: "clusterctl",
    macos: { brew: "clusterctl" },
    linux: { brew: "clusterctl" },
    // No first-party winget manifest as of 2026-08; upstream ships
    // release binaries at
    // https://github.com/kubernetes-sigs/cluster-api/releases.
    depends_on: &["kubectl"],
    category: "devops",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clusterctl_registration_shape() {
        assert_eq!(CLUSTERCTL.command, "clusterctl");
        assert_eq!(CLUSTERCTL.macos.expect("macOS").brew, Some("clusterctl"));
        assert_eq!(CLUSTERCTL.linux.expect("Linux").brew, Some("clusterctl"));
        assert!(CLUSTERCTL.windows.is_none(), "no first-party winget yet");
        assert!(
            CLUSTERCTL
                .depends_on
                .expect("declares deps")
                .contains(&"kubectl")
        );
    }
}
