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
    fn ensure_micro_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
