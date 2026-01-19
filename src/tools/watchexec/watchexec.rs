//! watchexec - execute commands in response to file changes
//!
//! watchexec is a simple, standalone tool that watches a path
//! and runs a command whenever it detects modifications.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(WATCHEXEC, {
    command: "watchexec",
    macos: { brew: "watchexec" },
    linux: { apt: "watchexec", dnf: "watchexec", pacman: "watchexec", apk: "watchexec" },
    windows: { winget: "watchexec.watchexec", choco: "watchexec" },
    bsd: { pkg: "watchexec" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_watchexec_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
