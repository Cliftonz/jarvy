//! crossplane — CLI for the Crossplane control plane, which extends
//! Kubernetes to provision and manage cloud infrastructure through
//! composable custom resources.
//!
//! The CLI builds/pushes packages and inspects a live control plane via
//! kubeconfig, so depends on `kubectl`.

use crate::define_tool;

define_tool!(CROSSPLANE, {
    command: "crossplane",
    macos: { brew: "crossplane" },
    linux: { brew: "crossplane" },
    // No first-party winget manifest as of 2026-08; upstream ships
    // release binaries — see https://docs.crossplane.io/latest/cli/.
    depends_on: &["kubectl"],
    category: "devops",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crossplane_registration_shape() {
        assert_eq!(CROSSPLANE.command, "crossplane");
        assert_eq!(CROSSPLANE.macos.expect("macOS").brew, Some("crossplane"));
        assert_eq!(CROSSPLANE.linux.expect("Linux").brew, Some("crossplane"));
        assert!(CROSSPLANE.windows.is_none(), "no first-party winget yet");
        assert!(
            CROSSPLANE
                .depends_on
                .expect("declares deps")
                .contains(&"kubectl")
        );
    }
}
