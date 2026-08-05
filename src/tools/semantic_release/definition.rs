//! semantic-release - automated semver + release publishing
//!
//! Derives the next semantic version from Conventional Commit messages,
//! generates release notes, and publishes packages/tags/GitHub releases
//! fully unattended. npm-registry-only distribution: no brew/apt/winget
//! package, so the PRD-060 npm fallback route installs it on every OS.

use crate::define_tool;

// define_tool!(SEMANTIC_RELEASE, …) stringifies as "semantic_release";
// registry dash/underscore aliasing means `semantic-release = "latest"`
// in jarvy.toml resolves too (same pattern as src/tools/nats_server).
define_tool!(SEMANTIC_RELEASE, {
    command: "semantic-release",
    // Ecosystem-only: upstream ships solely via the npm registry.
    // The fallback runtime bootstraps node through jarvy's own
    // registry when it's missing (verified 2026-08).
    fallback: { npm: "semantic-release" },
    category: "workflow",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_release_registration_shape() {
        assert_eq!(SEMANTIC_RELEASE.command, "semantic-release");
        assert_eq!(SEMANTIC_RELEASE.category, Some("workflow"));
        // no native package manager coverage; fallback route on all
        // platforms (verified 2026-08)
        assert!(SEMANTIC_RELEASE.macos.is_none());
        assert!(SEMANTIC_RELEASE.linux.is_none());
        assert!(SEMANTIC_RELEASE.windows.is_none());
        assert_eq!(SEMANTIC_RELEASE.fallback.len(), 1);
        assert_eq!(
            SEMANTIC_RELEASE.fallback[0].eco,
            crate::tools::spec::FallbackEco::Npm
        );
        assert_eq!(SEMANTIC_RELEASE.fallback[0].package, "semantic-release");
    }
}
