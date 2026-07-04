//! emqx - cloud-native MQTT 5.0 broker
//!
//! EMQX is a scalable, distributed MQTT 5.0 broker — the heaviest of
//! the MQTT options (Mosquitto is smaller / single-node; EMQX is
//! clustered out of the box). Especially common in IoT platforms
//! that need millions of concurrent connections.

use crate::define_tool;

define_tool!(EMQX, {
    command: "emqx",
    repo: "emqx/emqx",
    macos: { brew: "emqx" },
    linux: { uniform: "emqx" },
    // No first-party winget manifest as of 2026-06; download
    // installer from https://www.emqx.com/en/downloads.
    category: "messaging",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emqx_registration_shape() {
        assert_eq!(EMQX.command, "emqx");
        assert_eq!(EMQX.category, Some("messaging"));
        let mac = EMQX.macos.expect("emqx must support macOS");
        assert_eq!(mac.brew, Some("emqx"));
        let linux = EMQX.linux.expect("emqx must support Linux");
        assert_eq!(linux.apt, Some("emqx"));
        assert!(EMQX.windows.is_none(), "no first-party winget manifest");
    }
}
