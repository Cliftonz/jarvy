//! omnictl - Sidero Omni cluster management CLI
//!
//! omnictl is the CLI for Sidero Omni, a SaaS platform for managing
//! Talos Linux Kubernetes clusters across bare metal, cloud, and edge.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(OMNICTL, {
    command: "omnictl",
    macos: { brew: "siderolabs/tap/omnictl" },
    linux: { brew: "siderolabs/tap/omnictl" },
    windows: { winget: "Sidero.omnictl" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_omnictl_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
