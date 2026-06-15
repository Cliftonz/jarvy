//! podman_desktop - Podman Desktop GUI application
//!
//! This tool uses the ToolSpec pattern for declarative installation.
//! Podman Desktop provides a graphical interface for managing containers
//! and Kubernetes with Podman.

use crate::define_tool;

define_tool!(PODMAN_DESKTOP, {
    command: "podman-desktop",
    macos: { cask: "podman-desktop" },
    linux: { brew: "podman-desktop" },
    windows: { winget: "RedHat.Podman-Desktop" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn podman_desktop_registration_shape() {
        assert_eq!(PODMAN_DESKTOP.command, "podman-desktop");
        let mac = PODMAN_DESKTOP.macos.expect("must support macOS");
        assert_eq!(mac.cask, Some("podman-desktop"));
        let win = PODMAN_DESKTOP.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("RedHat.Podman-Desktop"));
    }
}
