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
    fn ensure_psql_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
