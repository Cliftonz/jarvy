//! arc - The Browser Company's Arc browser.
//!
//! macOS: Homebrew cask. Windows: winget (Windows build is in maintenance mode
//! per The Browser Company's May 2025 pivot to Dia — installer still ships).
//! Linux: never released.

use crate::define_tool;

define_tool!(ARC, {
    command: "arc",
    macos: { cask: "arc" },
    windows: { winget: "TheBrowserCompany.Arc" },
    category: "browser",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arc_registration_shape() {
        assert_eq!(ARC.command, "arc");
        assert_eq!(ARC.macos.unwrap().cask, Some("arc"));
        assert_eq!(ARC.windows.unwrap().winget, Some("TheBrowserCompany.Arc"));
        assert!(ARC.linux.is_none());
        assert_eq!(ARC.category, Some("browser"));
    }
}
