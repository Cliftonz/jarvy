//! kubeseal — client for Bitnami Sealed Secrets, a GitOps-safe secret
//! encryption controller.
//!
//! Encrypts a Kubernetes Secret into a SealedSecret CRD that only the
//! in-cluster controller can decrypt, so secrets can safely live in git.
//! Needs cluster access to fetch the controller's public certificate on
//! seal, so depends on `kubectl`.

use crate::define_tool;

define_tool!(KUBESEAL, {
    command: "kubeseal",
    macos: { brew: "kubeseal" },
    linux: { brew: "kubeseal" },
    // No first-party winget manifest as of 2026-08; upstream ships
    // release binaries at
    // https://github.com/bitnami-labs/sealed-secrets/releases.
    depends_on: &["kubectl"],
    category: "devops",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kubeseal_registration_shape() {
        assert_eq!(KUBESEAL.command, "kubeseal");
        assert_eq!(KUBESEAL.macos.expect("macOS").brew, Some("kubeseal"));
        assert_eq!(KUBESEAL.linux.expect("Linux").brew, Some("kubeseal"));
        assert!(KUBESEAL.windows.is_none(), "no first-party winget yet");
        assert!(
            KUBESEAL
                .depends_on
                .expect("declares deps")
                .contains(&"kubectl")
        );
    }
}
