//! helix - a post-modern modal text editor
//!
//! Helix is a Kakoune/Neovim inspired editor written in Rust.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(HELIX, {
    command: "hx",
    macos: { brew: "helix" },
    linux: { apt: "helix-editor", dnf: "helix", pacman: "helix", apk: "helix" },
    windows: { winget: "Helix.Helix" },
    bsd: { pkg: "helix" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helix_registration_shape() {
        assert_eq!(HELIX.command, "hx");
        let mac = HELIX.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("helix"));
        let win = HELIX.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Helix.Helix"));
    }
}
