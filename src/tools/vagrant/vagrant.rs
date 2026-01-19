//! vagrant - development environment automation
//!
//! Vagrant is a tool for building and distributing development environments.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(VAGRANT, {
    command: "vagrant",
    macos: { cask: "vagrant" },
    linux: { apt: "vagrant", dnf: "vagrant", pacman: "vagrant", apk: "vagrant" },
    windows: { winget: "Hashicorp.Vagrant", choco: "vagrant" },
    bsd: { pkg: "vagrant" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_vagrant_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
