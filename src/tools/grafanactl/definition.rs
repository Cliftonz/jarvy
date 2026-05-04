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
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_grafanactl_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
