//! netbird - Open-source WireGuard-based mesh VPN
//!
//! Open-source alternative to Tailscale. Distributed via the
//! `netbirdio/tap` Homebrew tap (auto-tapped by Jarvy when it sees the
//! `org/tap/formula` three-segment form) and the `Netbird.Netbird`
//! winget package.
//!
//! Linux note: NetBird publishes their own apt / dnf repos
//! (https://docs.netbird.io/how-to/installation). For users on linuxbrew
//! the brew tap works. Otherwise users follow upstream repo-setup
//! instructions manually.

use crate::define_tool;

define_tool!(NETBIRD, {
    command: "netbird",
    macos: { brew: "netbirdio/tap/netbird" },
    linux: { brew: "netbirdio/tap/netbird" },
    windows: { winget: "Netbird.Netbird" },
    category: "networking",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn netbird_registration_shape() {
        assert_eq!(NETBIRD.command, "netbird");
        let mac = NETBIRD.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("netbirdio/tap/netbird"));
        let lin = NETBIRD.linux.expect("must support Linux");
        assert_eq!(lin.brew, Some("netbirdio/tap/netbird"));
        let win = NETBIRD.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Netbird.Netbird"));
    }

    #[test]
    fn netbird_brew_form_triggers_auto_tap() {
        // org/tap/formula → Jarvy install path runs `brew tap org/tap`
        // before `brew install formula`. Verified by counting slashes.
        for spec in [
            NETBIRD.macos.unwrap().brew.unwrap(),
            NETBIRD.linux.unwrap().brew.unwrap(),
        ] {
            assert_eq!(
                spec.matches('/').count(),
                2,
                "brew spec must be org/tap/formula for auto-tap to fire"
            );
        }
    }
}
