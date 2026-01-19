//! usql - universal database CLI
//!
//! usql is a universal command-line interface for SQL databases.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(USQL, {
    command: "usql",
    macos: { brew: "usql" },
    linux: { brew: "usql" },
    windows: { choco: "usql" },
    bsd: { pkg: "usql" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_usql_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
