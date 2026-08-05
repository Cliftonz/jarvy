//! crane - container registry tool
//!
//! Crane is a tool for interacting with remote container images and registries.
//! It can copy, list, mutate, and inspect container images without a Docker daemon.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(CRANE, {
    command: "crane",
    macos: { brew: "crane" },
    linux: { uniform: "crane" },
    // No first-party winget manifest; the go fallback route covers
    // Windows (verified 2026-08).
    fallback: { go: "github.com/google/go-containerregistry/cmd/crane" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crane_registration_shape() {
        assert_eq!(CRANE.command, "crane");
        let mac = CRANE.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("crane"));
        assert!(CRANE.windows.is_none(), "no first-party winget manifest");
        assert_eq!(CRANE.fallback.len(), 1);
        assert_eq!(CRANE.fallback[0].eco, crate::tools::spec::FallbackEco::Go);
        assert_eq!(
            CRANE.fallback[0].package,
            "github.com/google/go-containerregistry/cmd/crane"
        );
    }
}
