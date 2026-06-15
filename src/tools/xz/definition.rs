//! xz - compression utility
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(XZ, {
    command: "xz",
    macos: { brew: "xz" },
    linux: { uniform: "xz" },
    windows: { winget: "XZUtils.XZ" },
    bsd: { pkg: "xz" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xz_registration_shape() {
        assert_eq!(XZ.command, "xz");
        let mac = XZ.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("xz"));
        let win = XZ.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("XZUtils.XZ"));
    }
}
