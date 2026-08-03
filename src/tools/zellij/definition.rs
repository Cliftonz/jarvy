//! zellij — terminal workspace / multiplexer with built-in layouts,
//! panes, and session resurrection.
//!
//! Windows support is first-party: upstream ships an MSI
//! (`zellij-x86_64-pc-windows-msvc-installer.msi`) that the
//! `Zellij.Zellij` winget manifest pins.

use crate::define_tool;

define_tool!(ZELLIJ, {
    command: "zellij",
    macos: { brew: "zellij" },
    linux: { brew: "zellij" },
    windows: { winget: "Zellij.Zellij" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zellij_registration_shape() {
        assert_eq!(ZELLIJ.command, "zellij");
        assert_eq!(ZELLIJ.macos.expect("macOS").brew, Some("zellij"));
        assert_eq!(ZELLIJ.linux.expect("Linux").brew, Some("zellij"));
        assert_eq!(
            ZELLIJ.windows.expect("Windows").winget,
            Some("Zellij.Zellij")
        );
        assert!(ZELLIJ.depends_on.is_none());
    }
}
