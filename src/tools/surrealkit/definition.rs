//! surrealkit — SurrealDB's official schema-management & migration CLI.
//!
//! Homepage: <https://github.com/surrealdb/surrealkit>. Migrations,
//! seeding, type-gen and testing for SurrealDB apps. Define the schema
//! as `.surql` files and keep the database in sync via two workflows:
//! `sync` (local/dev reconciliation) and `rollout` (shared/prod
//! migration). Supersedes the archived community `surrealdb-migrations`
//! CLI, which upstream now redirects to this tool.
//!
//! Install path: `cargo install --locked surrealkit`. Upstream also
//! ships `cargo binstall surrealkit` (prebuilt) and a Docker image
//! (`ghcr.io/surrealdb/surrealkit`), but there is no first-party
//! Homebrew / winget / choco / scoop / apt / dnf package as of 2026-07 —
//! crates.io is the canonical source. `depends_on: &["rust"]` puts the
//! toolchain (hence `cargo`) ahead of surrealkit in the topo sort, and
//! `--locked` respects the upstream `Cargo.lock` for supply-chain
//! integrity.

use crate::define_tool;
use crate::tools::common::{InstallError, install_via_cargo_install};

// Canonical publisher: SurrealDB — <https://crates.io/crates/surrealkit>.
// First-party (same org that ships the `surreal` binary). If crate
// ownership ever leaves the SurrealDB org, re-verify before shipping a
// bump — a hostile transfer would let a malicious binary reach every
// project whose setup installs this tool.
fn install_via_cargo(_min_hint: &str) -> Result<(), InstallError> {
    install_via_cargo_install("surrealkit")
}

define_tool!(SURREALKIT, {
    command: "surrealkit",
    repo: "surrealdb/surrealkit",
    custom_install: install_via_cargo,
    depends_on: &["rust"],
});

#[cfg(test)]
mod tests {
    use super::*;

    /// Pins the surrealkit shape: crates.io only (no verified first-party
    /// PM packaging), so every platform MUST fall through to the cargo
    /// custom_install path. If a future contributor adds a `macos.brew` /
    /// `windows.choco` slot without verifying upstream packaging, this
    /// test fails loudly.
    #[test]
    fn surrealkit_only_installs_via_cargo() {
        assert_eq!(SURREALKIT.command, "surrealkit");
        assert!(
            SURREALKIT.macos.is_none()
                && SURREALKIT.linux.is_none()
                && SURREALKIT.windows.is_none()
                && SURREALKIT.bsd.is_none(),
            "surrealkit ships on crates.io only; every platform must route \
             through custom_install (cargo)"
        );
        assert!(
            SURREALKIT.custom_install.is_some(),
            "surrealkit requires the cargo-install custom path"
        );
        assert_eq!(
            SURREALKIT.depends_on,
            Some(&["rust"] as &[&str]),
            "install path shells out to cargo — rust must be a dependency \
             so the topo sort installs the toolchain first"
        );
    }
}
