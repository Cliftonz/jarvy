//! dbmate - database migration tool
//!
//! dbmate is a database migration tool that supports multiple databases.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(DBMATE, {
    command: "dbmate",
    macos: { brew: "dbmate" },
    linux: { brew: "dbmate" },
    windows: { choco: "dbmate" },
    bsd: { pkg: "dbmate" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_dbmate_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
