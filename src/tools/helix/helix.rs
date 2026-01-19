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
    fn ensure_helix_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
