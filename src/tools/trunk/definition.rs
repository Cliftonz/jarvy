//! trunk — WASM web-application bundler for Rust (Leptos / Yew / Dioxus web).
//!
//! Homepage: <https://trunkrs.dev>. Provides a dev server with change
//! detection and browser reload, plus asset pipelines for Sass/Tailwind,
//! for Rust frontends targeting `wasm32-unknown-unknown`.
//!
//! Install path: `cargo install --locked trunk`. Do NOT wire a Homebrew
//! `trunk` formula: that name collides with trunk.io's meta-linter, so a
//! `brew install trunk` would install the wrong tool. `cargo install`
//! gives a single reproducible install across every platform Rust
//! supports — matches the pattern established by `bacon` /
//! `cargo-nextest` / `release-plz`.

use crate::define_tool;
use crate::tools::common::{InstallContext, InstallError, install_via_cargo_install};

// Canonical publisher: trunk-rs — <https://crates.io/crates/trunk>.
// Re-verify against the GitHub org (`trunk-rs/trunk`) on future bumps.
fn install_via_cargo(_min_hint: &str, _ctx: &InstallContext) -> Result<(), InstallError> {
    install_via_cargo_install("trunk")
}

define_tool!(TRUNK, {
    command: "trunk",
    custom_install: install_via_cargo,
    depends_on: &["rust"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trunk_uses_cargo_install() {
        assert_eq!(TRUNK.command, "trunk");
        assert!(TRUNK.custom_install.is_some());
        assert_eq!(TRUNK.depends_on, Some(&["rust"] as &[&str]));
    }

    /// Prevents a future contributor from wiring a Homebrew slot: the
    /// Homebrew formula named `trunk` is trunk.io's linter, not the
    /// Rust WASM bundler. Every platform MUST fall through to cargo.
    #[test]
    fn trunk_never_uses_first_party_package_managers() {
        assert!(
            TRUNK.macos.is_none()
                && TRUNK.linux.is_none()
                && TRUNK.windows.is_none()
                && TRUNK.bsd.is_none(),
            "trunk (WASM bundler) name collides with trunk.io on brew — \
             every platform must route through custom_install (cargo)"
        );
    }
}
