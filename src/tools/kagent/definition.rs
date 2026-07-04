//! kagent - Kubernetes-native AI agent framework
//!
//! kagent (CNCF Sandbox) provides an engine for building, deploying, and managing
//! AI agents on Kubernetes with built-in MCP server tools for K8s, Istio, Helm,
//! Argo, Prometheus, and more.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(KAGENT, {
    command: "kagent",
    repo: "kagent-dev/kagent",
    macos: { brew: "kagent" },
    linux: { brew: "kagent" },
    depends_on: &["kubectl"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kagent_registration_shape() {
        assert_eq!(KAGENT.command, "kagent");
        let mac = KAGENT.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("kagent"));
    }
}
