//! mssql-cli - Microsoft SQL Server interactive CLI
//!
//! Microsoft's `mssql-cli` is a Python-based interactive shell for SQL
//! Server with autocompletion + syntax highlighting. Distinct from
//! `sqlcmd` (the legacy non-interactive variant). Common companion to
//! EF Core / ASP.NET Core dev workflows where the developer needs an
//! ad-hoc query session against the project's local SQL Server.

use crate::define_tool;

define_tool!(MSSQL_CLI, {
    command: "mssql-cli",
    repo: "dbcli/mssql-cli",
    macos: { brew: "mssql-cli" },
    linux: { uniform: "mssql-cli" },
    windows: { winget: "Microsoft.SqlServer.MssqlCli" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mssql_cli_registration_shape() {
        assert_eq!(MSSQL_CLI.command, "mssql-cli");
        let mac = MSSQL_CLI.macos.expect("mssql-cli must support macOS");
        assert_eq!(mac.brew, Some("mssql-cli"));
    }
}
