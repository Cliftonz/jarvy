//! kubent (kube-no-trouble) — scans a live cluster for API resources
//! deployed against deprecated or removed Kubernetes API versions, so
//! upgrades don't break workloads mid-flight.
//!
//! Reads cluster state via kubeconfig, so depends on `kubectl`.

use crate::define_tool;

define_tool!(KUBENT, {
    command: "kubent",
    macos: { brew: "kubent" },
    linux: { brew: "kubent" },
    // No first-party winget manifest as of 2026-08; upstream ships
    // release binaries at
    // https://github.com/doitintl/kube-no-trouble/releases.
    depends_on: &["kubectl"],
    category: "devops",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kubent_registration_shape() {
        assert_eq!(KUBENT.command, "kubent");
        assert_eq!(KUBENT.macos.expect("macOS").brew, Some("kubent"));
        assert_eq!(KUBENT.linux.expect("Linux").brew, Some("kubent"));
        assert!(KUBENT.windows.is_none(), "no first-party winget yet");
        assert!(
            KUBENT
                .depends_on
                .expect("declares deps")
                .contains(&"kubectl")
        );
    }
}
