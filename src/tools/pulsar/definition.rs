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
    macos: { brew: "apache-pulsar" },
    linux: { uniform: "apache-pulsar" },
    windows: { winget: "Apache.Pulsar" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pulsar_registration_shape() {
        assert_eq!(PULSAR.command, "pulsar");
        let mac = PULSAR.macos.expect("pulsar must support macOS");
        assert_eq!(mac.brew, Some("apache-pulsar"));
    }
}
