//! yazi - blazing fast terminal file manager
//!
//! Yazi is a terminal file manager written in Rust with async I/O.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(YAZI, {
    command: "yazi",
    macos: { brew: "yazi" },
    linux: { apt: "yazi", dnf: "yazi", pacman: "yazi", apk: "yazi" },
    windows: { winget: "sxyazi.yazi" },
    bsd: { pkg: "yazi" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_yazi_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
