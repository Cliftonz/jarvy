//! yazi - blazing fast terminal file manager
//!
//! Yazi is a terminal file manager written in Rust with async I/O.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(YAZI, {
    command: "yazi",
    repo: "sxyazi/yazi",
    macos: { brew: "yazi" },
    linux: { apt: "yazi", dnf: "yazi", pacman: "yazi", apk: "yazi" },
    windows: { winget: "sxyazi.yazi" },
    bsd: { pkg: "yazi" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yazi_registration_shape() {
        assert_eq!(YAZI.command, "yazi");
        let mac = YAZI.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("yazi"));
        let win = YAZI.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("sxyazi.yazi"));
    }
}
