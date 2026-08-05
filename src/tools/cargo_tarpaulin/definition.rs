//! cargo-tarpaulin - Rust code-coverage tool
//!
//! Cargo subcommand that measures test coverage for Rust projects.
//! Full ptrace instrumentation is Linux-first; on other OSes tarpaulin
//! falls back to the llvm coverage engine. Distributed via crates.io —
//! no brew/apt/winget package, so the PRD-060 cargo fallback route is
//! the install path on every OS.

use crate::define_tool;

define_tool!(CARGO_TARPAULIN, {
    command: "cargo-tarpaulin",
    // Ecosystem-only: upstream ships via crates.io. The fallback
    // runtime bootstraps rustup/cargo through jarvy's own registry
    // when missing (verified 2026-08).
    fallback: { cargo: "cargo-tarpaulin" },
    category: "testing",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cargo_tarpaulin_registration_shape() {
        assert_eq!(CARGO_TARPAULIN.command, "cargo-tarpaulin");
        assert_eq!(CARGO_TARPAULIN.category, Some("testing"));
        // no native package manager coverage; fallback route on all
        // platforms (verified 2026-08)
        assert!(CARGO_TARPAULIN.macos.is_none());
        assert!(CARGO_TARPAULIN.linux.is_none());
        assert!(CARGO_TARPAULIN.windows.is_none());
        assert_eq!(CARGO_TARPAULIN.fallback.len(), 1);
        assert_eq!(
            CARGO_TARPAULIN.fallback[0].eco,
            crate::tools::spec::FallbackEco::Cargo
        );
        assert_eq!(CARGO_TARPAULIN.fallback[0].package, "cargo-tarpaulin");
    }
}
