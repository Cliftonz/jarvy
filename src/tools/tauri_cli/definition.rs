//! tauri-cli — build tool for Tauri desktop / mobile apps.
//!
//! Homepage: <https://tauri.app>. Rust + web-frontend alternative to
//! Electron built on the OS-native webview. Ships `cargo tauri init`,
//! `cargo tauri dev`, and `cargo tauri build` for cross-platform
//! desktop/mobile bundles. Installed binary is `cargo-tauri` (invoked as
//! the `cargo tauri` subcommand).
//!
//! Install path: `cargo install --locked tauri-cli`. No first-party
//! Homebrew / winget / apt / dnf packaging exists for the CLI as of
//! 2026-08 (the Tauri docs themselves recommend `cargo install`). Every
//! user is by definition a Rust developer, so route uniformly through
//! cargo like `bacon` / `cargo-nextest` / `release-plz`.

use crate::define_tool;
use crate::tools::common::{InstallContext, InstallError, install_via_cargo_install};

// Canonical publisher: tauri-apps — <https://crates.io/crates/tauri-cli>.
// Re-verify against the GitHub org (`tauri-apps/tauri`) on future
// bumps; tauri apps have full FS/network scope on end-user machines, so
// a hostile crate transfer has an unusually large blast radius.
fn install_via_cargo(_min_hint: &str, _ctx: &InstallContext) -> Result<(), InstallError> {
    install_via_cargo_install("tauri-cli")
}

define_tool!(TAURI_CLI, {
    command: "cargo-tauri",
    custom_install: install_via_cargo,
    depends_on: &["rust"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tauri_cli_uses_cargo_install() {
        assert_eq!(TAURI_CLI.command, "cargo-tauri");
        assert!(TAURI_CLI.custom_install.is_some());
        assert_eq!(TAURI_CLI.depends_on, Some(&["rust"] as &[&str]));
    }
}
