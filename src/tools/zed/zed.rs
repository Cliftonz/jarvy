//! zed - high-performance, multiplayer code editor
//!
//! Zed is a lightning-fast code editor from the creators of Atom and Tree-sitter.
//! Note: Linux install via official installer at zed.dev
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(ZED, {
    command: "zed",
    macos: { cask: "zed" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_zed_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
