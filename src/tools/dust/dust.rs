//! dust - a more intuitive version of du
//!
//! dust is a modern replacement for du written in Rust.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(DUST, {
    command: "dust",
    macos: { brew: "dust" },
    linux: { apt: "du-dust", dnf: "dust", pacman: "dust", apk: "dust" },
    windows: { winget: "bootandy.dust", choco: "dust" },
    bsd: { pkg: "dust" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_dust_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
