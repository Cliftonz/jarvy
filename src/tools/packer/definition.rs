//! packer - machine image builder
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(PACKER, {
    command: "packer",
    macos: { brew: "packer" },
    linux: { uniform: "packer" },
    windows: { winget: "HashiCorp.Packer" },
    bsd: { pkg: "packer" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packer_registration_shape() {
        assert_eq!(PACKER.command, "packer");
        let mac = PACKER.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("packer"));
        let win = PACKER.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("HashiCorp.Packer"));
    }
}
