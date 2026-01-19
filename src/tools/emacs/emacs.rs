//! emacs - extensible text editor
//!
//! GNU Emacs is an extensible, customizable, free text editor and computing environment.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(EMACS, {
    command: "emacs",
    macos: { cask: "emacs" },
    linux: { apt: "emacs", dnf: "emacs", pacman: "emacs", apk: "emacs" },
    windows: { winget: "GNU.Emacs" },
    bsd: { pkg: "emacs" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_emacs_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
