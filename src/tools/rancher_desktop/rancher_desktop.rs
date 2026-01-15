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
    fn ensure_rancher_desktop_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
