//! micro - modern and intuitive terminal-based text editor
//!
//! micro is a terminal-based text editor that aims to be easy to use and intuitive.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(MICRO, {
    command: "micro",
    macos: { brew: "micro" },
    linux: { apt: "micro", dnf: "micro", pacman: "micro", apk: "micro" },
    windows: { winget: "zyedidia.micro" },
    bsd: { pkg: "micro" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn micro_registration_shape() {
        assert_eq!(MICRO.command, "micro");
        let mac = MICRO.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("micro"));
        let win = MICRO.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("zyedidia.micro"));
    }
}
