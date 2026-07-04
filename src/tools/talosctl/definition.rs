//! talosctl - Talos Linux cluster management CLI
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(TALOSCTL, {
    command: "talosctl",
    repo: "siderolabs/talos",
    macos: { brew: "siderolabs/tap/talosctl" },
    linux: { uniform: "talosctl" },
    windows: { winget: "SideroLabs.talosctl" },
    bsd: { pkg: "talosctl" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn talosctl_registration_shape() {
        assert_eq!(TALOSCTL.command, "talosctl");
        let mac = TALOSCTL.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("siderolabs/tap/talosctl"));
        let win = TALOSCTL.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("SideroLabs.talosctl"));
    }
}
