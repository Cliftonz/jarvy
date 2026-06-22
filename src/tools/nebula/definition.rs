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
    }
}
