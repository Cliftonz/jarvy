//! caddy - web server with automatic HTTPS
//!
//! Serves sites and reverse-proxies local services, provisioning TLS certificates automatically.

use crate::define_tool;

define_tool!(CADDY, {
    command: "caddy",
    macos: { brew: "caddy" },
    linux: { apt: "caddy", dnf: "caddy", pacman: "caddy", apk: "caddy" },
    windows: { winget: "CaddyServer.Caddy" },
    category: "networking",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caddy_registration_shape() {
        assert_eq!(CADDY.command, "caddy");
        assert_eq!(CADDY.category, Some("networking"));
        let mac = CADDY.macos.expect("caddy must support macOS");
        assert_eq!(mac.brew, Some("caddy"));
        let linux = CADDY.linux.expect("caddy must support Linux");
        assert_eq!(linux.apt, Some("caddy"));
        assert_eq!(linux.dnf, Some("caddy"));
        assert_eq!(linux.pacman, Some("caddy"));
        assert_eq!(linux.apk, Some("caddy"));
        let win = CADDY.windows.expect("caddy must support Windows");
        assert_eq!(win.winget, Some("CaddyServer.Caddy"));
        assert!(CADDY.fallback.is_empty());
        assert!(CADDY.bsd.is_none());
    }
}
