//! autohotkey - AutoHotkey, scripting language and automation tool for
//! Windows. Hotkeys, hotstrings, GUI automation, remapping keyboard/mouse.
//!
//! Windows: winget (`AutoHotkey.AutoHotkey`) / choco (`autohotkey`).
//! macOS/Linux: not released (Windows-only by design).

use crate::define_tool;

define_tool!(AUTOHOTKEY, {
    command: "AutoHotkey",
    windows: { winget: "AutoHotkey.AutoHotkey", choco: "autohotkey" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autohotkey_registration_shape() {
        assert_eq!(AUTOHOTKEY.command, "AutoHotkey");
        let win = AUTOHOTKEY.windows.unwrap();
        assert_eq!(win.winget, Some("AutoHotkey.AutoHotkey"));
        assert_eq!(win.choco, Some("autohotkey"));
        assert!(AUTOHOTKEY.macos.is_none());
        assert!(AUTOHOTKEY.linux.is_none());
    }
}
