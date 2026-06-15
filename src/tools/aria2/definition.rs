//! aria2 - lightweight multi-protocol download utility
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(ARIA2, {
    command: "aria2c",
    macos: { brew: "aria2" },
    linux: { uniform: "aria2" },
    windows: { winget: "aria2.aria2" },
    bsd: { pkg: "aria2" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aria2_registration_shape() {
        assert_eq!(ARIA2.command, "aria2c");
        let mac = ARIA2.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("aria2"));
        let win = ARIA2.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("aria2.aria2"));
    }
}
