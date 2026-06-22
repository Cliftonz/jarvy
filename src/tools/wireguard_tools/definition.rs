//! wireguard-tools - Userspace utilities for the WireGuard VPN
//!
//! Ships the `wg` and `wg-quick` CLIs. On Linux the in-kernel WireGuard
//! module pairs with this package; on macOS and Windows users typically
//! get the GUI client (also via winget) which bundles `wg.exe`.

use crate::define_tool;

define_tool!(WIREGUARD_TOOLS, {
    command: "wg",
    macos: { brew: "wireguard-tools" },
    linux: { uniform: "wireguard-tools" },
    windows: { winget: "WireGuard.WireGuard" },
    category: "networking",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wireguard_tools_registration_shape() {
        assert_eq!(WIREGUARD_TOOLS.command, "wg");
        let mac = WIREGUARD_TOOLS.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("wireguard-tools"));
        let lin = WIREGUARD_TOOLS.linux.expect("must support Linux");
        assert_eq!(lin.apt, Some("wireguard-tools"));
        let win = WIREGUARD_TOOLS.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("WireGuard.WireGuard"));
    }
}
