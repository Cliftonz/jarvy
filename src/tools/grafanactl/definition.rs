//! grafanactl - Grafana CLI
//!
//! Grafanactl is a CLI tool for interacting with Grafana instances.
//! It enables authentication, environment management, and administrative
//! tasks through Grafana's REST API from the terminal.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(GRAFANACTL, {
    command: "grafanactl",
    macos: { brew: "grafanactl" },
    linux: { uniform: "grafanactl" },
    // No first-party winget manifest; the go fallback route covers
    // Windows (verified 2026-08).
    fallback: { go: "github.com/grafana/grafanactl/cmd/grafanactl" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grafanactl_registration_shape() {
        assert_eq!(GRAFANACTL.command, "grafanactl");
        let mac = GRAFANACTL.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("grafanactl"));
        assert!(
            GRAFANACTL.windows.is_none(),
            "no first-party winget manifest"
        );
        assert_eq!(GRAFANACTL.fallback.len(), 1);
        assert_eq!(
            GRAFANACTL.fallback[0].eco,
            crate::tools::spec::FallbackEco::Go
        );
        assert_eq!(
            GRAFANACTL.fallback[0].package,
            "github.com/grafana/grafanactl/cmd/grafanactl"
        );
    }
}
