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
use crate::tools::common::{InstallError, has, run};

fn install_via_cargo(_min_hint: &str) -> Result<(), InstallError> {
    if !has("cargo") {
        return Err(InstallError::Prereq(
            "cargo not found — install the Rust toolchain first \
             (add `rust = \"latest\"` under `[provisioner]` and \
             re-run `jarvy setup`).",
        ));
    }
    run("cargo", &["install", "--locked", "bacon"])?;
    Ok(())
}

define_tool!(BACON, {
    command: "bacon",
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
