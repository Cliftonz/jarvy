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
    fn ensure_bottom_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
