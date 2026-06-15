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
    fn sqlite_registration_shape() {
        assert_eq!(SQLITE.command, "sqlite3");
        let mac = SQLITE.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("sqlite"));
        let win = SQLITE.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("SQLite.SQLite"));
    }
}
