//! vagrant - development environment automation
//!
//! Vagrant is a tool for building and distributing development environments.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(VAGRANT, {
    command: "vagrant",
    repo: "hashicorp/vagrant",
    macos: { cask: "vagrant" },
    linux: { apt: "vagrant", dnf: "vagrant", pacman: "vagrant", apk: "vagrant" },
    windows: { winget: "Hashicorp.Vagrant", choco: "vagrant" },
    bsd: { pkg: "vagrant" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vagrant_registration_shape() {
        assert_eq!(VAGRANT.command, "vagrant");
        let mac = VAGRANT.macos.expect("must support macOS");
        assert_eq!(mac.cask, Some("vagrant"));
        let win = VAGRANT.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Hashicorp.Vagrant"));
    }
}
