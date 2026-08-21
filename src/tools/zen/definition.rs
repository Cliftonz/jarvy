//! zen - Zen Browser (Firefox fork focused on productivity).
//!
//! macOS: Homebrew cask. Windows: winget. Linux: not wired in v1 — upstream
//! ships Flathub `app.zen_browser.zen` and AppImage/tarball from GitHub
//! releases (`zen-browser/desktop`); no distro-package or vendor apt/dnf
//! repo. Add via the flatpak / AppImage path in a follow-up.

use crate::define_tool;

define_tool!(ZEN, {
    command: "zen-browser",
    macos: { cask: "zen" },
    windows: { winget: "Zen-Team.Zen-Browser" },
    category: "browser",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zen_registration_shape() {
        assert_eq!(ZEN.command, "zen-browser");
        assert_eq!(ZEN.macos.unwrap().cask, Some("zen"));
        assert_eq!(ZEN.windows.unwrap().winget, Some("Zen-Team.Zen-Browser"));
        assert!(ZEN.linux.is_none());
        assert_eq!(ZEN.category, Some("browser"));
    }
}
