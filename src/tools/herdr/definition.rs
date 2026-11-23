//! herdr - terminal multiplexer for AI coding agents

use crate::define_tool;

define_tool!(HERDR, {
    command: "herdr",
    // brew formula + winget id verified 2026-11.
    macos: { brew: "herdr" },
    // No distro package as of 2026-11; Linuxbrew serves the homebrew-core formula.
    linux: { brew: "herdr" },
    windows: { winget: "Herdr.Herdr" },
    // No cargo fallback: the crates.io herdr crate is a stale 0.1.0 under a different license (checked 2026-11).
    category: "ai-agent",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn herdr_registration_shape() {
        assert_eq!(HERDR.command, "herdr");
        let mac = HERDR.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("herdr"));
        let linux = HERDR.linux.expect("must support Linux");
        assert_eq!(linux.brew, Some("herdr"));
        let win = HERDR.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Herdr.Herdr"));
        assert!(HERDR.fallback.is_empty());
        assert_eq!(HERDR.category, Some("ai-agent"));
    }
}
