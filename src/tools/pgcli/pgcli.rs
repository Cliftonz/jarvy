//! pgcli - postgres CLI with auto-completion
//!
//! pgcli is a command line interface for Postgres with auto-completion
//! and syntax highlighting.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(PGCLI, {
    command: "pgcli",
    macos: { brew: "pgcli" },
    linux: { apt: "pgcli", dnf: "pgcli", pacman: "pgcli", apk: "pgcli" },
    windows: { choco: "pgcli" },
    bsd: { pkg: "py39-pgcli" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_pgcli_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
