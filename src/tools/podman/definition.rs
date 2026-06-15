//! podman - Daemonless container engine
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(PODMAN, {
    command: "podman",
    macos: { brew: "podman" },
    linux: { apt: "podman", dnf: "podman", pacman: "podman", apk: "podman" },
    windows: { winget: "RedHat.Podman" },
    bsd: { pkg: "podman" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn podman_registration_shape() {
        assert_eq!(PODMAN.command, "podman");
        let mac = PODMAN.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("podman"));
        let win = PODMAN.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("RedHat.Podman"));
    }
}
