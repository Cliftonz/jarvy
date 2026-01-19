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
    fn ensure_podman_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
