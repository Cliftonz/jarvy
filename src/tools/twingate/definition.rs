//! twingate - Zero-trust remote access client
//!
//! Winget id is `Twingate.Client` (the desktop client), NOT
//! `Twingate.Twingate` (which doesn't exist in the catalog as of
//! 2026-06-22).
//!
//! Linux note: Twingate Linux client requires their own apt/dnf repo
//! (https://www.twingate.com/docs/linux-installation). No first-party
//! brew formula. Linux block intentionally omitted — Jarvy emits
//! `tool.unsupported` and the user installs manually.

use crate::define_tool;

define_tool!(TWINGATE, {
    command: "twingate",
    macos: { cask: "twingate" },
    windows: { winget: "Twingate.Client" },
    category: "networking",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn twingate_registration_shape() {
        assert_eq!(TWINGATE.command, "twingate");
        let mac = TWINGATE.macos.expect("must support macOS");
        assert_eq!(mac.cask, Some("twingate"));
        let win = TWINGATE.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Twingate.Client"));
        assert!(TWINGATE.linux.is_none());
    }
}
