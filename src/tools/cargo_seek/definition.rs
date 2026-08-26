//! cargo-seek — TUI for searching, adding, and installing cargo crates.
//!
//! Homepage: <https://crates.io/crates/cargo-seek>. Wraps `cargo search`,
//! `cargo add`, `cargo info`, and `cargo install` behind an interactive
//! terminal UI, so users can browse crates.io without leaving the shell.
//!
//! Install path: `cargo install --locked cargo-seek`. No first-party
//! Homebrew / winget / apt / dnf packaging as of 2026-08. Every user of
//! cargo-seek is by definition a Rust developer, so `cargo` is
//! guaranteed available — matches the pattern established by
//! `cargo-nextest` / `bacon` / `release-plz`.

use crate::define_tool;
use crate::tools::common::{InstallContext, InstallError, install_via_cargo_install};

// Canonical publisher: <https://crates.io/crates/cargo-seek>. Verify the
// crates.io owner on future bumps — a hostile ownership transfer would
// ship a malicious binary the next time Jarvy runs `cargo install
// --locked cargo-seek` for a Rust project.
fn install_via_cargo(_min_hint: &str, _ctx: &InstallContext) -> Result<(), InstallError> {
    install_via_cargo_install("cargo-seek")
}

define_tool!(CARGO_SEEK, {
    command: "cargo-seek",
    custom_install: install_via_cargo,
    depends_on: &["rust"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cargo_seek_uses_cargo_install() {
        assert_eq!(CARGO_SEEK.command, "cargo-seek");
        assert!(CARGO_SEEK.custom_install.is_some());
        assert_eq!(CARGO_SEEK.depends_on, Some(&["rust"] as &[&str]));
    }
}
