//! openvpn - Open-source VPN daemon + client
//!
//! Maps to the OSS OpenVPN community CLI (winget id
//! `OpenVPNTechnologies.OpenVPN`). For the proprietary OpenVPN Connect
//! GUI client use `OpenVPNTechnologies.OpenVPNConnect` directly via
//! winget — Jarvy ships the OSS one here because it has a real `openvpn`
//! CLI in PATH that the `has()` detection model can verify.

use crate::define_tool;

define_tool!(OPENVPN, {
    command: "openvpn",
    macos: { brew: "openvpn" },
    linux: { uniform: "openvpn" },
    windows: { winget: "OpenVPNTechnologies.OpenVPN" },
    category: "networking",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openvpn_registration_shape() {
        assert_eq!(OPENVPN.command, "openvpn");
        let mac = OPENVPN.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("openvpn"));
        let lin = OPENVPN.linux.expect("must support Linux");
        assert_eq!(lin.apt, Some("openvpn"));
        let win = OPENVPN.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("OpenVPNTechnologies.OpenVPN"));
    }
}
