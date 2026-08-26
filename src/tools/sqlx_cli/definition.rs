//! sqlx-cli — command-line companion to the SQLx Rust SQL toolkit.
//!
//! Homepage: <https://github.com/launchbadge/sqlx/tree/main/sqlx-cli>.
//! Provides `sqlx database {create,drop}`, `sqlx migrate {add,run,revert}`,
//! and offline query metadata (`cargo sqlx prepare`) for compile-time
//! query verification. Installed binary is named `sqlx`.
//!
//! Install path: `cargo install --locked sqlx-cli`. Homebrew ships
//! `sqlx-cli` on macOS but no first-party winget / apt / dnf packaging
//! exists as of 2026-08. Every user is by definition a Rust project with
//! a SQL dependency, so route uniformly through cargo like `bacon` /
//! `cargo-nextest` / `release-plz`.

use crate::define_tool;
use crate::tools::common::{InstallContext, InstallError, install_via_cargo_install};

// Canonical publisher: launchbadge — <https://crates.io/crates/sqlx-cli>.
// Owner has not changed since sqlx v0.1. Re-verify against the GitHub
// org (`launchbadge/sqlx`) on future bumps; a hostile transfer would
// ship a malicious binary that has database credentials in scope.
fn install_via_cargo(_min_hint: &str, _ctx: &InstallContext) -> Result<(), InstallError> {
    install_via_cargo_install("sqlx-cli")
}

define_tool!(SQLX_CLI, {
    command: "sqlx",
    custom_install: install_via_cargo,
    depends_on: &["rust"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlx_cli_uses_cargo_install() {
        assert_eq!(SQLX_CLI.command, "sqlx");
        assert!(SQLX_CLI.custom_install.is_some());
        assert_eq!(SQLX_CLI.depends_on, Some(&["rust"] as &[&str]));
    }
}
