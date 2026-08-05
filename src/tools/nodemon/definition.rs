//! nodemon - Node.js dev auto-restart watcher (remy/nodemon)
//!
//! Watches source files during development and restarts the Node.js
//! process on change — the de facto dev-loop companion to `node`.
//! npm-registry-only distribution: no brew/apt/winget package, so the
//! PRD-060 npm fallback route installs it on every OS.

use crate::define_tool;

define_tool!(NODEMON, {
    command: "nodemon",
    // Ecosystem-only: upstream ships solely via the npm registry.
    // The fallback runtime bootstraps node through jarvy's own
    // registry when it's missing (verified 2026-08).
    fallback: { npm: "nodemon" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nodemon_registration_shape() {
        assert_eq!(NODEMON.command, "nodemon");
        // no native package manager coverage; fallback route on all
        // platforms (verified 2026-08)
        assert!(NODEMON.macos.is_none());
        assert!(NODEMON.linux.is_none());
        assert!(NODEMON.windows.is_none());
        assert_eq!(NODEMON.fallback.len(), 1);
        assert_eq!(
            NODEMON.fallback[0].eco,
            crate::tools::spec::FallbackEco::Npm
        );
        assert_eq!(NODEMON.fallback[0].package, "nodemon");
    }
}
