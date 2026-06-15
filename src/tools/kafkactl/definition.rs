//! kafkactl - declarative Kafka admin CLI (deviceinsight)
//!
//! `kafkactl` is a `kubectl`-shaped CLI for Kafka — declarative
//! `kafkactl apply -f topic.yaml`, `kafkactl get topics`,
//! `kafkactl describe consumer-group`. Often preferred over `kcat`
//! when admin work dominates over message debugging.

use crate::define_tool;

define_tool!(KAFKACTL, {
    command: "kafkactl",
    macos: { brew: "deviceinsight/packages/kafkactl" },
    linux: { uniform: "kafkactl" },
    // No first-party winget manifest; install via release binary.
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kafkactl_registration_shape() {
        assert_eq!(KAFKACTL.command, "kafkactl");
        let mac = KAFKACTL.macos.expect("kafkactl must support macOS");
        assert_eq!(
            mac.brew,
            Some("deviceinsight/packages/kafkactl"),
            "macOS formula lives in the deviceinsight/packages tap"
        );
    }
}
