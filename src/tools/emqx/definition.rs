//! emqx - cloud-native MQTT 5.0 broker
//!
//! EMQX is a scalable, distributed MQTT 5.0 broker — the heaviest of
//! the MQTT options (Mosquitto is smaller / single-node; EMQX is
//! clustered out of the box). Especially common in IoT platforms
//! that need millions of concurrent connections.

use crate::define_tool;

define_tool!(EMQX, {
    command: "emqx",
    macos: { brew: "emqx" },
    linux: { uniform: "emqx" },
    // No first-party winget manifest; download installer from emqx.io.
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emqx_registration_shape() {
        assert_eq!(EMQX.command, "emqx");
        let mac = EMQX.macos.expect("emqx must support macOS");
        assert_eq!(mac.brew, Some("emqx"));
    }
}
