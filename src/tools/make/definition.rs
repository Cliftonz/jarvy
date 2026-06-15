//! make - build automation tool
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(MAKE, {
    command: "make",
    macos: { brew: "make" },
    linux: { uniform: "make" },
    windows: { winget: "GnuWin32.Make" },
    bsd: { pkg: "gmake" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn make_registration_shape() {
        assert_eq!(MAKE.command, "make");
        let mac = MAKE.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("make"));
        let win = MAKE.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("GnuWin32.Make"));
    }
}
