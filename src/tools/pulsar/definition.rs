//! pulsar - Apache Pulsar distributed pub/sub + queuing platform
//!
//! Apache Pulsar combines the durability + partitioning of Kafka with
//! the queue semantics of RabbitMQ, plus multi-tenancy, tiered
//! storage, and georeplication. The brew formula ships the broker
//! (`pulsar`) plus admin/client CLIs (`pulsar-admin`, `pulsar-client`,
//! `pulsar-perf`).

use crate::define_tool;

define_tool!(PULSAR, {
    command: "pulsar",
    repo: "apache/pulsar",
    macos: { brew: "apache-pulsar" },
    // Linux: no native distro package; install via Linuxbrew.
    linux: { brew: "apache-pulsar" },
    // No first-party winget manifest as of 2026-06; the prior
    // `Apache.Pulsar` id was never claimed. Windows users: install
    // from https://pulsar.apache.org/download/.
    category: "messaging",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pulsar_registration_shape() {
        assert_eq!(PULSAR.command, "pulsar");
        assert_eq!(PULSAR.category, Some("messaging"));
        let mac = PULSAR.macos.expect("pulsar must support macOS");
        assert_eq!(mac.brew, Some("apache-pulsar"));
        let linux = PULSAR.linux.expect("pulsar must support Linux");
        assert_eq!(linux.brew, Some("apache-pulsar"));
        assert!(PULSAR.windows.is_none(), "no first-party winget manifest");
    }
}
