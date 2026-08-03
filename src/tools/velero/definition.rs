//! velero — backup, restore, and disaster-recovery for Kubernetes
//! resources and persistent volumes.
//!
//! CLI drives the in-cluster velero server, so depends on `kubectl` for
//! kubeconfig context. Upstream ships release binaries; brew is the
//! supported install for macOS and Linux.

use crate::define_tool;

define_tool!(VELERO, {
    command: "velero",
    macos: { brew: "velero" },
    linux: { brew: "velero" },
    // No first-party winget manifest as of 2026-08; upstream ships
    // release binaries at https://github.com/vmware-tanzu/velero/releases.
    depends_on: &["kubectl"],
    category: "devops",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn velero_registration_shape() {
        assert_eq!(VELERO.command, "velero");
        assert_eq!(VELERO.macos.expect("macOS").brew, Some("velero"));
        assert_eq!(VELERO.linux.expect("Linux").brew, Some("velero"));
        assert!(VELERO.windows.is_none(), "no first-party winget yet");
        assert!(
            VELERO
                .depends_on
                .expect("declares deps")
                .contains(&"kubectl")
        );
    }
}
