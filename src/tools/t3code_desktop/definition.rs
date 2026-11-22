//! t3code_desktop - T3 Code desktop app (GUI only; the CLI is the separate t3code tool)

use crate::define_tool;

define_tool!(T3CODE_DESKTOP, {
    command: "t3code-desktop",
    // brew cask + winget verified 2026-11, both pull from github.com/pingdotgg/t3code releases.
    macos: { cask: "t3-code" },
    // No Linux package-manager route as of 2026-11; upstream ships only AppImage and deb files.
    windows: { winget: "T3Tools.T3Code" },
    category: "ai-agent",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t3code_desktop_registration_shape() {
        assert_eq!(T3CODE_DESKTOP.command, "t3code-desktop");
        assert_eq!(T3CODE_DESKTOP.macos.unwrap().cask, Some("t3-code"));
        assert!(T3CODE_DESKTOP.linux.is_none());
        assert_eq!(
            T3CODE_DESKTOP.windows.unwrap().winget,
            Some("T3Tools.T3Code")
        );
        assert!(T3CODE_DESKTOP.fallback.is_empty());
        assert_eq!(T3CODE_DESKTOP.category, Some("ai-agent"));
    }
}
