//! dbmate - database migration tool
//!
//! dbmate is a database migration tool that supports multiple databases.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(DBMATE, {
    command: "dbmate",
    repo: "amacneil/dbmate",
    macos: { brew: "dbmate" },
    linux: { brew: "dbmate" },
    windows: { choco: "dbmate" },
    bsd: { pkg: "dbmate" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dbmate_registration_shape() {
        assert_eq!(DBMATE.command, "dbmate");
        let mac = DBMATE.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("dbmate"));
    }
}
