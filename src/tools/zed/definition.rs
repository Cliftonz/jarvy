//! zed - high-performance, multiplayer code editor
//!
//! Zed is a lightning-fast code editor from the creators of Atom and Tree-sitter.
//! Note: Linux install via official installer at zed.dev
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(ZED, {
    command: "zed",
    repo: "zed-industries/zed",
    macos: { cask: "zed" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zed_registration_shape() {
        assert_eq!(ZED.command, "zed");
        let mac = ZED.macos.expect("must support macOS");
        assert_eq!(mac.cask, Some("zed"));
    }
}
