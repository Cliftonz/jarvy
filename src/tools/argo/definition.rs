//! argo - Argo Workflows CLI (Kubernetes-native workflow queue)
//!
//! Argo Workflows is a Kubernetes-native workflow / DAG engine — the
//! `argo` CLI submits / inspects / retries workflows running on a
//! cluster. Pairs with NATS / Kafka / Temporal as the "job queue +
//! orchestration" layer in event-driven backends.

use crate::define_tool;

define_tool!(ARGO, {
    command: "argo",
    macos: { brew: "argo" },
    linux: { uniform: "argo" },
    windows: { winget: "Argoproj.Argo" },
    depends_on_one_of: &["kubectl"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argo_registration_shape() {
        assert_eq!(ARGO.command, "argo");
        let mac = ARGO.macos.expect("argo must support macOS");
        assert_eq!(mac.brew, Some("argo"));
        let win = ARGO.windows.expect("argo must support Windows");
        assert_eq!(win.winget, Some("Argoproj.Argo"));
    }
}
