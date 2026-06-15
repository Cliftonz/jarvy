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
    fn usql_registration_shape() {
        assert_eq!(USQL.command, "usql");
        let mac = USQL.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("usql"));
    }
}
