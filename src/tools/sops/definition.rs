//! sops - Secrets OPerationS - manage encrypted files
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(SOPS, {
    command: "sops",
    repo: "getsops/sops",
    macos: { brew: "sops" },
    linux: { brew: "sops" },
    windows: { winget: "Mozilla.sops" },
    bsd: { pkg: "sops" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sops_registration_shape() {
        assert_eq!(SOPS.command, "sops");
        let mac = SOPS.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("sops"));
        let win = SOPS.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Mozilla.sops"));
    }
}
