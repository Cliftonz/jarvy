//! shfmt - Shell script formatter
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(SHFMT, {
    command: "shfmt",
    macos: { brew: "shfmt" },
    linux: { uniform: "shfmt" },
    windows: { winget: "mvdan.shfmt" },
    bsd: { pkg: "shfmt" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shfmt_registration_shape() {
        assert_eq!(SHFMT.command, "shfmt");
        let mac = SHFMT.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("shfmt"));
        let win = SHFMT.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("mvdan.shfmt"));
    }
}
