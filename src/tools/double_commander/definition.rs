//! double_commander - Double Commander (alexx2000), open-source cross-platform
//! dual-pane file manager inspired by Total Commander. GPL-2.0.
//!
//! Windows: winget (`alexx2000.DoubleCommander`).
//! macOS: Homebrew cask (`double-commander`).
//! Linux: omitted — Debian/Ubuntu ship `doublecmd-gtk` / `doublecmd-qt` (UI
//! variants), Fedora has `doublecmd-gtk` / `doublecmd-qt5`, others differ.
//! Users can pick their toolkit variant via `[apt]` / `[dnf]` overrides.

use crate::define_tool;

define_tool!(DOUBLE_COMMANDER, {
    command: "doublecmd",
    macos: { cask: "double-commander" },
    windows: { winget: "alexx2000.DoubleCommander" },
    category: "file_manager",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn double_commander_registration_shape() {
        assert_eq!(DOUBLE_COMMANDER.command, "doublecmd");
        assert_eq!(
            DOUBLE_COMMANDER.macos.unwrap().cask,
            Some("double-commander")
        );
        assert_eq!(
            DOUBLE_COMMANDER.windows.unwrap().winget,
            Some("alexx2000.DoubleCommander")
        );
        assert!(
            DOUBLE_COMMANDER.linux.is_none(),
            "distro package names diverge (doublecmd-gtk / -qt / -qt5) — omit rather than guess"
        );
        assert_eq!(DOUBLE_COMMANDER.category, Some("file_manager"));
    }
}
