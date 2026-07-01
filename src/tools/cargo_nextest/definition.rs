//! cargo-nextest — next-generation test runner for Rust.
//!
//! Homepage: <https://nexte.st>. Roughly 2-3× faster than the built-in
//! `cargo test` on large workspaces (parallel execution, per-test
//! process isolation, better failure reporting).
//!
//! Install path: `cargo install --locked cargo-nextest`. Homebrew ships
//! a formula (`cargo-nextest`) but its bottle is macOS-only, and no
//! first-party winget / apt / dnf packaging exists. Since every user
//! of cargo-nextest is by definition a Rust project, cargo is
//! guaranteed available and gives us a single reproducible install
//! path across every platform Rust supports — matches the pattern
//! established by `release-plz`.

use crate::define_tool;
use crate::tools::common::{InstallError, has, run};

fn install_via_cargo(_min_hint: &str) -> Result<(), InstallError> {
    if !has("cargo") {
        return Err(InstallError::Prereq(
            "cargo not found — install the Rust toolchain first \
             (add `rust = \"latest\"` under `[provisioner]` and \
             re-run `jarvy setup`).",
        ));
    }
    run("cargo", &["install", "--locked", "cargo-nextest"])?;
    Ok(())
}

define_tool!(CARGO_NEXTEST, {
    command: "cargo-nextest",
    custom_install: install_via_cargo,
    depends_on: &["rust"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cargo_nextest_uses_cargo_install() {
        assert_eq!(CARGO_NEXTEST.command, "cargo-nextest");
        assert!(
            CARGO_NEXTEST.custom_install.is_some(),
            "must route through cargo install path"
        );
        assert_eq!(CARGO_NEXTEST.depends_on, Some(&["rust"] as &[&str]));
    }
}
