//! cilium — installs, manages, and diagnoses Cilium networking (CNI +
//! service mesh), Hubble observability, and ClusterMesh federation.
//!
//! The upstream package is `cilium-cli` (brew formula, winget publisher);
//! the shipped binary is `cilium`. Tool name matches the binary so users
//! write `cilium = "latest"` in `jarvy.toml`.
//! Depends on `kubectl` — every operation targets a kubeconfig context.

use crate::define_tool;

define_tool!(CILIUM, {
    command: "cilium",
    macos: { brew: "cilium-cli" },
    linux: { brew: "cilium-cli" },
    windows: { winget: "Cilium.CiliumCLI" },
    depends_on: &["kubectl"],
    category: "devops",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cilium_registration_shape() {
        assert_eq!(CILIUM.command, "cilium");
        assert_eq!(CILIUM.macos.expect("macOS").brew, Some("cilium-cli"));
        assert_eq!(CILIUM.linux.expect("Linux").brew, Some("cilium-cli"));
        assert_eq!(
            CILIUM.windows.expect("Windows").winget,
            Some("Cilium.CiliumCLI")
        );
        assert!(
            CILIUM
                .depends_on
                .expect("declares deps")
                .contains(&"kubectl")
        );
    }
}
