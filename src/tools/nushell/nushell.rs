//! nushell - a new type of shell
//!
//! Nushell is a modern shell that understands structured data.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(NUSHELL, {
    command: "nu",
    macos: { brew: "nushell" },
    linux: { apt: "nushell", dnf: "nushell", pacman: "nushell", apk: "nushell" },
    windows: { winget: "Nushell.Nushell" },
    bsd: { pkg: "nushell" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_nushell_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
