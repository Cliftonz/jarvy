//! blisk - Blisk browser (dev-focused Chromium with mobile-preview split).
//!
//! macOS: Homebrew cask. No first-party winget manifest as of 2026-08 —
//! install from https://blisk.io/. Linux: not distributed by upstream.

use crate::define_tool;

define_tool!(BLISK, {
    command: "blisk",
    macos: { cask: "blisk" },
    category: "browser",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blisk_registration_shape() {
        assert_eq!(BLISK.command, "blisk");
        assert_eq!(BLISK.macos.unwrap().cask, Some("blisk"));
        assert!(
            BLISK.windows.is_none(),
            "no first-party winget manifest — omit rather than claim"
        );
        assert!(BLISK.linux.is_none());
        assert_eq!(BLISK.category, Some("browser"));
    }
}
