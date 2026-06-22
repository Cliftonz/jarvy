//! zerotier - Software-defined networking / global mesh VPN
//!
//! ZeroTier ships as a daemon (`zerotier-one`) with a CLI shim
//! (`zerotier-cli`). The macOS cask + Windows winget package install both;
//! the `command` field tracks the CLI shim that lives on PATH.
//!
//! Linux note: no first-party brew formula and no default-distro package.
//! Users on Linux follow ZeroTier's repo-setup instructions
//! (https://www.zerotier.com/download/). Linux block intentionally
//! omitted — Jarvy emits `tool.unsupported` and the user installs
//! manually.

use crate::define_tool;

define_tool!(ZEROTIER, {
    command: "zerotier-cli",
    macos: { cask: "zerotier-one" },
    windows: { winget: "ZeroTier.ZeroTierOne" },
    category: "networking",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zerotier_registration_shape() {
        assert_eq!(ZEROTIER.command, "zerotier-cli");
        let mac = ZEROTIER.macos.expect("must support macOS");
        assert_eq!(mac.cask, Some("zerotier-one"));
        let win = ZEROTIER.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("ZeroTier.ZeroTierOne"));
        assert!(ZEROTIER.linux.is_none());
    }
}
