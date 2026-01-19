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
    fn ensure_packer_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
