//! tailscale - Zero-config mesh VPN built on WireGuard
//!
//! Tailscale connects machines into a private mesh network without
//! exposing public ports. CLI client + tailscaled daemon.
//!
//! Linux note: the canonical Linux install path is Tailscale's own apt /
//! dnf repos (see https://tailscale.com/download). For users on
//! linuxbrew we fall back to the brew formula; users with their distro's
//! native package manager and no linuxbrew should follow upstream's
//! repo-setup instructions manually.

use crate::define_tool;

define_tool!(TAILSCALE, {
    command: "tailscale",
    macos: { brew: "tailscale" },
    linux: { brew: "tailscale" },
    windows: { winget: "Tailscale.Tailscale" },
    category: "networking",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tailscale_registration_shape() {
        assert_eq!(TAILSCALE.command, "tailscale");
        let mac = TAILSCALE.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("tailscale"));
        let lin = TAILSCALE.linux.expect("must support Linux");
        assert_eq!(lin.brew, Some("tailscale"));
        let win = TAILSCALE.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Tailscale.Tailscale"));
    }
}
