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
    fn nushell_registration_shape() {
        assert_eq!(NUSHELL.command, "nu");
        let mac = NUSHELL.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("nushell"));
        let win = NUSHELL.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Nushell.Nushell"));
    }
}
