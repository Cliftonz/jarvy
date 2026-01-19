//! btop - Resource monitor with TUI
//!
//! This tool uses the ToolSpec pattern for declarative installation.
//! Note: No Windows support available.

use crate::define_tool;

define_tool!(BTOP, {
    command: "btop",
    macos: { brew: "btop" },
    linux: { apt: "btop", dnf: "btop", pacman: "btop", apk: "btop" },
    bsd: { pkg: "btop" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_btop_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
