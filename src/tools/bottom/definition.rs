//! bottom - Cross-platform graphical process/system monitor
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(BOTTOM, {
    command: "btm",
    macos: { brew: "bottom" },
    linux: { uniform: "bottom" },
    windows: { winget: "Clement.bottom" },
    bsd: { pkg: "bottom" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bottom_registration_shape() {
        assert_eq!(BOTTOM.command, "btm");
        let mac = BOTTOM.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("bottom"));
        let win = BOTTOM.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Clement.bottom"));
    }
}
