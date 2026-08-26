//! cargo-generate — scaffold new Rust projects from a git template repo.
//!
//! Homepage: <https://cargo-generate.github.io/cargo-generate>. Renders
//! a template repo (e.g. `leptos-rs/start`) into a fresh Rust project
//! with placeholder substitution, so teams can standardize the shape of
//! a new service/app/library without copy-paste.
//!
//! Install path: `cargo install --locked cargo-generate`. Homebrew ships
//! a formula but it is macOS-only; no first-party winget / apt / dnf
//! packaging as of 2026-08. Every user of cargo-generate is by
//! definition a Rust developer, so route uniformly through cargo like
//! `bacon` / `cargo-nextest` / `release-plz`.

use crate::define_tool;
use crate::tools::common::{InstallContext, InstallError, install_via_cargo_install};

// Canonical publisher: cargo-generate maintainers —
// <https://crates.io/crates/cargo-generate>. Re-verify the crates.io
// owner on future bumps against the GitHub org (`cargo-generate/cargo-generate`).
fn install_via_cargo(_min_hint: &str, _ctx: &InstallContext) -> Result<(), InstallError> {
    install_via_cargo_install("cargo-generate")
}

define_tool!(CARGO_GENERATE, {
    command: "cargo-generate",
    custom_install: install_via_cargo,
    depends_on: &["rust"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cargo_generate_uses_cargo_install() {
        assert_eq!(CARGO_GENERATE.command, "cargo-generate");
        assert!(CARGO_GENERATE.custom_install.is_some());
        assert_eq!(CARGO_GENERATE.depends_on, Some(&["rust"] as &[&str]));
    }
}
