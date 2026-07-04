//! btop - Resource monitor with TUI
//!
//! This tool uses the ToolSpec pattern for declarative installation.
//! Note: No Windows support available.

use crate::define_tool;

define_tool!(BTOP, {
    command: "btop",
    repo: "aristocratos/btop",
    macos: { brew: "btop" },
    linux: { apt: "btop", dnf: "btop", pacman: "btop", apk: "btop" },
    bsd: { pkg: "btop" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn btop_registration_shape() {
        assert_eq!(BTOP.command, "btop");
        let mac = BTOP.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("btop"));
    }
}
