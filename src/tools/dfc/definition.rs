//! dfc - Dockerfile converter for Chainguard images
//!
//! A CLI utility that converts Dockerfiles to use Chainguard Images and APKs.
//! Facilitates migration to secure, minimal base images by automatically
//! replacing standard base images with their Chainguard equivalents.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(DFC, {
    command: "dfc",
    macos: { brew: "chainguard-dev/tap/dfc" },
    linux: { brew: "chainguard-dev/tap/dfc" },
    // No first-party winget manifest; the go fallback route covers
    // Windows — module root, README-documented go install
    // (verified 2026-08).
    fallback: { go: "github.com/chainguard-dev/dfc" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dfc_registration_shape() {
        assert_eq!(DFC.command, "dfc");
        let mac = DFC.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("chainguard-dev/tap/dfc"));
        assert!(DFC.windows.is_none(), "no first-party winget manifest");
        assert_eq!(DFC.fallback.len(), 1);
        assert_eq!(DFC.fallback[0].eco, crate::tools::spec::FallbackEco::Go);
        assert_eq!(DFC.fallback[0].package, "github.com/chainguard-dev/dfc");
    }
}
