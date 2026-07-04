//! bacon — background Rust code check runner.
//!
//! Homepage: <https://dystroy.org/bacon>. Modern successor to
//! `cargo-watch`: reruns `cargo check` / `clippy` / `test` on file
//! change, presents results in a persistent TUI (no scrollback churn),
//! and re-scopes to the failing test on failure.
//!
//! Install path: `cargo install --locked bacon`. Homebrew ships bacon
//! but the formula is macOS-only and no first-party winget / apt / dnf
//! packaging exists. Same rationale as `cargo-nextest` / `release-plz`
//! — every user has cargo by definition, so route uniformly.

use crate::define_tool;
use crate::tools::common::{InstallError, install_via_cargo_install};

// Canonical publisher: Denys Séguret / dystroy — <https://crates.io/crates/bacon>.
// If the crates.io ownership changes upstream, revisit this pin: a
// hostile transfer would ship a malicious binary the next time Jarvy
// runs `cargo install --locked bacon` for a Rust project. As of
// 2026-07 the owner is unchanged since the crate's first release.
fn install_via_cargo(_min_hint: &str) -> Result<(), InstallError> {
    install_via_cargo_install("bacon")
}

define_tool!(BACON, {
    command: "bacon",
    repo: "Canop/bacon",
    custom_install: install_via_cargo,
    depends_on: &["rust"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bacon_uses_cargo_install() {
        assert_eq!(BACON.command, "bacon");
        assert!(BACON.custom_install.is_some());
        assert_eq!(BACON.depends_on, Some(&["rust"] as &[&str]));
    }
}
