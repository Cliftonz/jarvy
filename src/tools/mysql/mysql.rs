//! mysql - MySQL command-line client
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(MYSQL, {
    command: "mysql",
    macos: { brew: "mysql-client" },
    linux: { apt: "mysql-client", dnf: "mysql", pacman: "mysql", apk: "mysql-client" },
    windows: { winget: "Oracle.MySQL" },
    bsd: { pkg: "mysql80-client" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_mysql_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
