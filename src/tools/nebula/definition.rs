//! nebula - Slack's scalable overlay networking tool
//!
//! Lighthouse-coordinated mesh VPN with mutual-TLS-style certificates.
//! The brew formula is cross-platform (macOS + linuxbrew) and ships both
//! `nebula` and `nebula-cert` binaries; the `command` field tracks
//! `nebula` for has()-detection.
//!
//! Windows note: no first-party winget manifest as of 2026-06-22 and
//! Slack does not publish a Windows .msi. Users on Windows download
//! `nebula-windows-amd64.zip` from
//! https://github.com/slackhq/nebula/releases and place the binaries on
//! PATH manually.

use crate::define_tool;

define_tool!(NEBULA, {
    command: "nebula",
    macos: { brew: "nebula" },
    linux: { brew: "nebula" },
    // No first-party winget manifest; the go fallback route covers
    // Windows (verified 2026-08). Caveat: installs only the `nebula`
    // binary — `nebula-cert` (which brew bundles) is not covered.
    fallback: { go: "github.com/slackhq/nebula/cmd/nebula" },
    category: "networking",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nebula_registration_shape() {
        assert_eq!(NEBULA.command, "nebula");
        let mac = NEBULA.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("nebula"));
        let lin = NEBULA.linux.expect("must support Linux");
        assert_eq!(lin.brew, Some("nebula"));
        // Windows intentionally omitted — see module doc.
        assert!(NEBULA.windows.is_none());
        assert_eq!(NEBULA.fallback.len(), 1);
        assert_eq!(NEBULA.fallback[0].eco, crate::tools::spec::FallbackEco::Go);
        assert_eq!(
            NEBULA.fallback[0].package,
            "github.com/slackhq/nebula/cmd/nebula"
        );
    }
}
