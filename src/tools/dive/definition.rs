//! dive - Docker image layer explorer
//!
//! A tool for exploring a docker image, layer contents, and discovering ways to shrink the image.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(DIVE, {
    command: "dive",
    macos: { brew: "dive" },
    linux: { brew: "dive", apk: "dive" },
    bsd: { pkg: "dive" },
    // No first-party winget manifest; the go fallback route covers
    // Windows — main package at module root, README-documented
    // (verified 2026-08).
    fallback: { go: "github.com/wagoodman/dive" },
    // dive can explore images from docker or podman
    depends_on_one_of: &["docker", "podman"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dive_registration_shape() {
        assert_eq!(DIVE.command, "dive");
        let mac = DIVE.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("dive"));
        assert!(DIVE.windows.is_none(), "no first-party winget manifest");
        assert_eq!(DIVE.fallback.len(), 1);
        assert_eq!(DIVE.fallback[0].eco, crate::tools::spec::FallbackEco::Go);
        assert_eq!(DIVE.fallback[0].package, "github.com/wagoodman/dive");
    }
}
