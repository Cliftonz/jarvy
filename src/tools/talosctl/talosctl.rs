//! talosctl - Talos Linux cluster management CLI
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(TALOSCTL, {
    command: "talosctl",
    macos: { brew: "siderolabs/tap/talosctl" },
    linux: { uniform: "talosctl" },
    windows: { winget: "SideroLabs.talosctl" },
    bsd: { pkg: "talosctl" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_talosctl_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
