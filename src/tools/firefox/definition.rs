//! firefox - Mozilla Firefox browser.
//!
//! macOS: Homebrew cask. Windows: winget. Linux: available in every mainstream
//! distro's default repos as `firefox`, so we route through the uniform slot
//! rather than bootstrap the Mozilla APT repo. Users who want the
//! Mozilla-signed channel can install manually.

use crate::define_tool;

define_tool!(FIREFOX, {
    command: "firefox",
    macos: { cask: "firefox" },
    linux: { uniform: "firefox" },
    windows: { winget: "Mozilla.Firefox" },
    category: "browser",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn firefox_registration_shape() {
        assert_eq!(FIREFOX.command, "firefox");
        assert_eq!(FIREFOX.macos.unwrap().cask, Some("firefox"));
        assert_eq!(FIREFOX.linux.unwrap().apt, Some("firefox"));
        assert_eq!(FIREFOX.linux.unwrap().dnf, Some("firefox"));
        assert_eq!(FIREFOX.windows.unwrap().winget, Some("Mozilla.Firefox"));
        assert_eq!(FIREFOX.category, Some("browser"));
    }
}
