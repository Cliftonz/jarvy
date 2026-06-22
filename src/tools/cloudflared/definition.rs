//! cloudflared - Cloudflare Tunnel + Warp daemon
//!
//! CLI for creating Cloudflare Tunnels (zero-trust ingress) and running
//! the Warp client. Cloudflare's brew formula is cross-platform; on Linux
//! without linuxbrew, users add Cloudflare's apt/dnf repo per
//! https://pkg.cloudflare.com (not auto-configured here).

use crate::define_tool;

define_tool!(CLOUDFLARED, {
    command: "cloudflared",
    macos: { brew: "cloudflared" },
    linux: { brew: "cloudflared" },
    windows: { winget: "Cloudflare.cloudflared" },
    category: "networking",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloudflared_registration_shape() {
        assert_eq!(CLOUDFLARED.command, "cloudflared");
        let mac = CLOUDFLARED.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("cloudflared"));
        let lin = CLOUDFLARED.linux.expect("must support Linux");
        assert_eq!(lin.brew, Some("cloudflared"));
        let win = CLOUDFLARED.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Cloudflare.cloudflared"));
    }
}
