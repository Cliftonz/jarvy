//! litecli - sqlite CLI with auto-completion
//!
//! litecli is a command line interface for SQLite with auto-completion
//! and syntax highlighting.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(LITECLI, {
    command: "litecli",
    macos: { brew: "litecli" },
    linux: { apt: "litecli", dnf: "litecli", pacman: "litecli", apk: "litecli" },
    windows: { choco: "litecli" },
    bsd: { pkg: "py39-litecli" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_litecli_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
