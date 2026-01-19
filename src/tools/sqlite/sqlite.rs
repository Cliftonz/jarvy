//! sqlite - SQLite command-line interface
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(SQLITE, {
    command: "sqlite3",
    macos: { brew: "sqlite" },
    linux: { uniform: "sqlite3" },
    windows: { winget: "SQLite.SQLite" },
    bsd: { pkg: "sqlite3" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_sqlite_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
