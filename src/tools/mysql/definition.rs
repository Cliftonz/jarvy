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
    fn mysql_registration_shape() {
        assert_eq!(MYSQL.command, "mysql");
        let mac = MYSQL.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("mysql-client"));
        let win = MYSQL.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Oracle.MySQL"));
    }
}
