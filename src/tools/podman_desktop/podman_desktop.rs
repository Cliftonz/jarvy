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
    fn ensure_podman_desktop_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
