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
    fn ensure_duckdb_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
