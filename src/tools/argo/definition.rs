//! argo - Argo Workflows CLI (Kubernetes-native workflow queue)
//!
//! Argo Workflows is a Kubernetes-native workflow / DAG engine — the
//! `argo` CLI submits / inspects / retries workflows running on a
//! cluster. Pairs with NATS / Kafka / Temporal as the "job queue +
//! orchestration" layer in event-driven backends.

use crate::define_tool;

define_tool!(ARGO, {
    command: "argo",
    repo: "argoproj/argo-workflows",
    macos: { brew: "argo" },
    // Linux: install via Linuxbrew or release binary; no native
    // distro package (`argo` as a Debian package is an unrelated
    // legacy tool, so resolve via brew to avoid namespace squat).
    linux: { brew: "argo" },
    // No first-party winget manifest as of 2026-06; the prior
    // `Argoproj.Argo` id was never claimed. Windows users: install
    // from https://github.com/argoproj/argo-workflows/releases.
    depends_on_one_of: &["kubectl"],
    category: "workflow",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argo_registration_shape() {
        assert_eq!(ARGO.command, "argo");
        assert_eq!(ARGO.category, Some("workflow"));
        let mac = ARGO.macos.expect("argo must support macOS");
        assert_eq!(mac.brew, Some("argo"));
        let linux = ARGO.linux.expect("argo must support Linux");
        assert_eq!(linux.brew, Some("argo"));
        assert!(ARGO.windows.is_none(), "no first-party winget manifest");
    }
}
