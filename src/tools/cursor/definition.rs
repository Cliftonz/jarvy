//! cursor - AI-first code editor
//!
//! Cursor is an AI-powered code editor built on VS Code.
//! Note: Linux install via AppImage from cursor.sh
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(CURSOR, {
    command: "cursor",
    macos: { cask: "cursor" },
    windows: { winget: "Cursor.Cursor" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_registration_shape() {
        assert_eq!(CURSOR.command, "cursor");
        let mac = CURSOR.macos.expect("must support macOS");
        assert_eq!(mac.cask, Some("cursor"));
        let win = CURSOR.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Cursor.Cursor"));
    }
}
