//! pm2 - Node.js production process manager (Unitech/pm2)
//!
//! Daemonizes Node.js apps with load balancing, auto-restart on crash,
//! log management, and `pm2 startup` boot persistence. Distributed
//! exclusively through npm — no brew/apt/winget package exists, so the
//! PRD-060 npm fallback route is the install path on every OS.

use crate::define_tool;

define_tool!(PM2, {
    command: "pm2",
    // Ecosystem-only: upstream ships solely via the npm registry.
    // The fallback runtime bootstraps node through jarvy's own
    // registry when it's missing (verified 2026-08).
    fallback: { npm: "pm2" },
    // Node.js process manager — daemonizes Node apps at runtime.
    depends_on_one_of: &["node", "nvm"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pm2_registration_shape() {
        assert_eq!(PM2.command, "pm2");
        // no native package manager coverage; fallback route on all
        // platforms (verified 2026-08)
        assert!(PM2.macos.is_none());
        assert!(PM2.linux.is_none());
        assert!(PM2.windows.is_none());
        assert_eq!(PM2.fallback.len(), 1);
        assert_eq!(PM2.fallback[0].eco, crate::tools::spec::FallbackEco::Npm);
        assert_eq!(PM2.fallback[0].package, "pm2");
    }

    #[test]
    fn pm2_needs_node_runtime() {
        assert_eq!(PM2.depends_on_one_of, Some(&["node", "nvm"] as &[&str]));
    }
}
