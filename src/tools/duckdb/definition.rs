//! duckdb - analytical SQL database
//!
//! DuckDB is an in-process analytical database management system.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(DUCKDB, {
    command: "duckdb",
    macos: { brew: "duckdb" },
    linux: { apt: "duckdb", dnf: "duckdb", pacman: "duckdb", apk: "duckdb" },
    windows: { winget: "DuckDB.cli", choco: "duckdb" },
    bsd: { pkg: "duckdb" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duckdb_registration_shape() {
        assert_eq!(DUCKDB.command, "duckdb");
        let mac = DUCKDB.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("duckdb"));
        let win = DUCKDB.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("DuckDB.cli"));
    }
}
