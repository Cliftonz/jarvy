//! knip - unused-code finder for TS/JS projects (webpro-nl/knip)
//!
//! Finds unused files, exports, and dependencies in TypeScript and
//! JavaScript projects — the modern successor to depcheck/ts-prune.
//! npm-registry-only distribution: no brew/apt/winget package, so the
//! PRD-060 npm fallback route installs it on every OS.

use crate::define_tool;

define_tool!(KNIP, {
    command: "knip",
    // Ecosystem-only: upstream ships solely via the npm registry.
    // The fallback runtime bootstraps node through jarvy's own
    // registry when it's missing (verified 2026-08).
    fallback: { npm: "knip" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knip_registration_shape() {
        assert_eq!(KNIP.command, "knip");
        // no native package manager coverage; fallback route on all
        // platforms (verified 2026-08)
        assert!(KNIP.macos.is_none());
        assert!(KNIP.linux.is_none());
        assert!(KNIP.windows.is_none());
        assert_eq!(KNIP.fallback.len(), 1);
        assert_eq!(KNIP.fallback[0].eco, crate::tools::spec::FallbackEco::Npm);
        assert_eq!(KNIP.fallback[0].package, "knip");
    }
}
