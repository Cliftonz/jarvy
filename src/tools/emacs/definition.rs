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
    fn emacs_registration_shape() {
        assert_eq!(EMACS.command, "emacs");
        let mac = EMACS.macos.expect("must support macOS");
        assert_eq!(mac.cask, Some("emacs"));
        let win = EMACS.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("GNU.Emacs"));
    }
}
