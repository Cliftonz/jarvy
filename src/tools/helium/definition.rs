//! helium - ImputNet Helium browser (privacy-first, imputnet/helium-linux).
//!
//! Windows: winget. macOS: no cask yet — the existing `helium` cask on brew
//! is an unrelated koush project (deprecated 2026-09-01), so we deliberately
//! omit the macos slot rather than pin to it. Linux: AppImage / .deb / .rpm
//! from https://helium.computer/download — no vendor apt/dnf repo.
//!
//! Refresh: monitor https://helium.computer/ for a first-party macOS cask
//! and Linux repo before adding those slots.

use crate::define_tool;

define_tool!(HELIUM, {
    command: "helium",
    windows: { winget: "ImputNet.Helium" },
    category: "browser",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helium_registration_shape() {
        assert_eq!(HELIUM.command, "helium");
        assert_eq!(HELIUM.windows.unwrap().winget, Some("ImputNet.Helium"));
        assert!(
            HELIUM.macos.is_none(),
            "existing brew `helium` cask is unrelated koush project"
        );
        assert!(
            HELIUM.linux.is_none(),
            "no vendor repo — install from helium.computer"
        );
        assert_eq!(HELIUM.category, Some("browser"));
    }
}
