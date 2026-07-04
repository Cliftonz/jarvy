//! release-plz — Rust-native release automation (changelog + version
//! bump + `cargo publish` + GitHub Release), driven from CI.
//!
//! Homepage: <https://release-plz.dev>. Marker file `release-plz.toml`
//! at the repo root triggers `jarvy discover` to recommend the tool
//! (see the `release-plz` rule in `discover/rules.rs`).
//!
//! Install path: `cargo install --locked release-plz`. As of 2026-07
//! there is no first-party Homebrew formula, no winget/choco/scoop
//! package, and no apt/dnf packaging — only Alpine (`apk`) and Arch
//! (`pacman`) ship official builds. Since every release-plz user is by
//! definition a Rust project, `cargo` is guaranteed to be on the box
//! (declared via `depends_on: &["rust"]`), so routing every platform
//! through cargo gives a single reproducible install with `--locked`
//! respecting the upstream `Cargo.lock` for supply-chain integrity.

use crate::define_tool;
use crate::tools::common::{InstallError, install_via_cargo_install};

// Canonical publisher: Marco Ieni — <https://crates.io/crates/release-plz>.
// Owner unchanged since v0.1. If a future ownership transfer lands,
// re-verify the maintainer matches release-plz.dev before shipping
// a bump — a hostile transfer would let a malicious binary reach
// every Rust project whose wizard-run recommends release-plz.
fn install_via_cargo(_min_hint: &str) -> Result<(), InstallError> {
    install_via_cargo_install("release-plz")
}

define_tool!(RELEASE_PLZ, {
    command: "release-plz",
    repo: "release-plz/release-plz",
    custom_install: install_via_cargo,
    depends_on: &["rust"],
});

#[cfg(test)]
mod tests {
    use super::*;

    /// Pins the release-plz shape: no first-party package-manager
    /// coverage anywhere, so every platform MUST fall through to the
    /// cargo custom_install path. If a future contributor adds a
    /// `macos.brew` or `windows.winget` slot without also verifying
    /// the upstream publisher, this test fails loudly.
    #[test]
    fn release_plz_only_installs_via_cargo() {
        assert_eq!(RELEASE_PLZ.command, "release-plz");
        assert!(
            RELEASE_PLZ.macos.is_none()
                && RELEASE_PLZ.linux.is_none()
                && RELEASE_PLZ.windows.is_none()
                && RELEASE_PLZ.bsd.is_none(),
            "release-plz has no verified first-party PM packaging; \
             every platform must route through custom_install (cargo)"
        );
        assert!(
            RELEASE_PLZ.custom_install.is_some(),
            "release-plz requires the cargo-install custom path"
        );
        assert_eq!(
            RELEASE_PLZ.depends_on,
            Some(&["rust"] as &[&str]),
            "release-plz install path shells out to cargo — rust must \
             be declared as a dependency so the topo sort installs the \
             toolchain first"
        );
    }
}
