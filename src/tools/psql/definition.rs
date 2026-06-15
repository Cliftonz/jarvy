//! psql - PostgreSQL interactive terminal
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(PSQL, {
    command: "psql",
    macos: { brew: "postgresql" },
    linux: { apt: "postgresql-client", dnf: "postgresql", pacman: "postgresql", apk: "postgresql-client" },
    windows: { winget: "PostgreSQL.PostgreSQL" },
    bsd: { pkg: "postgresql16-client" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn psql_registration_shape() {
        assert_eq!(PSQL.command, "psql");
        let mac = PSQL.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("postgresql"));
        let win = PSQL.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("PostgreSQL.PostgreSQL"));
    }
}
