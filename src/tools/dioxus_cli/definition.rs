//! dioxus-cli — build tool for Dioxus fullstack web/desktop/mobile apps.
//!
//! Homepage: <https://dioxuslabs.com>. Provides `dx new`, `dx serve`
//! (hot reload), `dx build`, and `dx bundle` for iOS / Android / desktop
//! / web targets from a single Rust codebase. Installed binary is `dx`.
//!
//! Install path: `cargo install --locked dioxus-cli`. No first-party
//! Homebrew / winget / apt / dnf packaging as of 2026-08. Every user is
//! by definition a Rust developer, so route uniformly through cargo
//! like `bacon` / `cargo-nextest` / `release-plz`.

use crate::define_tool;
use crate::tools::common::{InstallContext, InstallError, install_via_cargo_install};

// Canonical publisher: DioxusLabs — <https://crates.io/crates/dioxus-cli>.
// Re-verify against the GitHub org (`DioxusLabs/dioxus`) on future bumps.
fn install_via_cargo(_min_hint: &str, _ctx: &InstallContext) -> Result<(), InstallError> {
    install_via_cargo_install("dioxus-cli")
}

define_tool!(DIOXUS_CLI, {
    command: "dx",
    custom_install: install_via_cargo,
    depends_on: &["rust"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dioxus_cli_uses_cargo_install() {
        assert_eq!(DIOXUS_CLI.command, "dx");
        assert!(DIOXUS_CLI.custom_install.is_some());
        assert_eq!(DIOXUS_CLI.depends_on, Some(&["rust"] as &[&str]));
    }
}
