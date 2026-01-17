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
    fn ensure_cursor_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
