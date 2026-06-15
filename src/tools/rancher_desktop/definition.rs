//! rancher_desktop - Kubernetes and container management desktop application
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(RANCHER_DESKTOP, {
    command: "rdctl",
    macos: { cask: "rancher" },
    linux: { brew: "rancher" },
    windows: { winget: "suse.RancherDesktop" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rancher_desktop_registration_shape() {
        assert_eq!(RANCHER_DESKTOP.command, "rdctl");
        let mac = RANCHER_DESKTOP.macos.expect("must support macOS");
        assert_eq!(mac.cask, Some("rancher"));
        let win = RANCHER_DESKTOP.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("suse.RancherDesktop"));
    }
}
