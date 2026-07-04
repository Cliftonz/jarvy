//! dapr - Distributed Application Runtime CLI
//!
//! Microsoft-incubated, CNCF-graduated runtime for building portable
//! event-driven microservices. Heavy adoption in .NET microservices
//! shops (the Dapr SDK for .NET is first-class) — `dapr init`,
//! `dapr run`, `dapr deploy` are the canonical local-dev workflow.

use crate::define_tool;

define_tool!(DAPR, {
    command: "dapr",
    repo: "dapr/cli",
    macos: { brew: "dapr/tap/dapr-cli" },
    linux: { uniform: "dapr" },
    windows: { winget: "Dapr.CLI" },
    depends_on_one_of: &["docker", "podman"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dapr_registration_shape() {
        assert_eq!(DAPR.command, "dapr");
        let mac = DAPR.macos.expect("dapr must support macOS");
        assert_eq!(
            mac.brew,
            Some("dapr/tap/dapr-cli"),
            "macOS formula lives in the dapr tap"
        );
        let win = DAPR.windows.expect("dapr must support Windows");
        assert_eq!(win.winget, Some("Dapr.CLI"));
    }
}
