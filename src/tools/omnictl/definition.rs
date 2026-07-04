//! omnictl - Sidero Omni cluster management CLI
//!
//! omnictl is the CLI for Sidero Omni, a SaaS platform for managing
//! Talos Linux Kubernetes clusters across bare metal, cloud, and edge.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(OMNICTL, {
    command: "omnictl",
    repo: "siderolabs/omni",
    macos: { brew: "siderolabs/tap/omnictl" },
    linux: { brew: "siderolabs/tap/omnictl" },
    windows: { winget: "Sidero.omnictl" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn omnictl_registration_shape() {
        assert_eq!(OMNICTL.command, "omnictl");
        let mac = OMNICTL.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("siderolabs/tap/omnictl"));
        let win = OMNICTL.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Sidero.omnictl"));
    }
}
