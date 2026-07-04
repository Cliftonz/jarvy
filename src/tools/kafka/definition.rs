//! kafka - Apache Kafka distributed event streaming platform
//!
//! Apache Kafka itself — broker, ZooKeeper (legacy) / KRaft (modern)
//! controller, and the bundled CLI scripts (`kafka-console-producer`,
//! `kafka-console-consumer`, `kafka-topics`, etc.). Most real-world
//! Kafka work uses lighter CLIs (`kcat`, `rpk`, `kaf`) for everyday
//! pub/sub; install this when you need the full broker for local
//! development or want the official admin scripts.
//!
//! The `command:` is `kafka-topics` (one of the bundled scripts) —
//! `kafka` itself isn't a binary, the distribution exposes its
//! functionality through ~30 separate scripts. Picking
//! `kafka-topics` because it's the most-invoked one.
//!
//! Note: drift detection runs `kafka-topics --version`; broker-only
//! installs without the bundled scripts will register as missing.

use crate::define_tool;

define_tool!(KAFKA, {
    command: "kafka-topics",
    repo: "apache/kafka",
    macos: { brew: "kafka" },
    // Linux: Apache Kafka isn't packaged in mainstream distros.
    // Install via Linuxbrew or the upstream tarball.
    linux: { brew: "kafka" },
    // No first-party winget manifest as of 2026-06; the prior
    // `Apache.Kafka` id was never claimed. Windows users: install
    // from https://kafka.apache.org/downloads.
    category: "messaging",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kafka_registration_shape() {
        assert_eq!(KAFKA.command, "kafka-topics");
        assert_eq!(KAFKA.category, Some("messaging"));
        let mac = KAFKA.macos.expect("kafka must support macOS");
        assert_eq!(mac.brew, Some("kafka"));
        let linux = KAFKA.linux.expect("kafka must support Linux");
        assert_eq!(linux.brew, Some("kafka"));
        assert!(KAFKA.windows.is_none(), "no first-party winget manifest");
    }
}
