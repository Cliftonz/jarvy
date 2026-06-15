//! kn - Knative CLI (serverless + event-driven on Kubernetes)
//!
//! `kn` is the canonical CLI for Knative Serving (serverless
//! request/response on K8s) and Knative Eventing (CloudEvents-based
//! event mesh). The eventing side is the queue/messaging-relevant
//! piece — channels, subscriptions, brokers, triggers over a
//! pluggable backend (NATS, Kafka, RabbitMQ).

use crate::define_tool;

define_tool!(KN, {
    command: "kn",
    macos: { brew: "kn" },
    linux: { uniform: "kn" },
    // No first-party winget manifest; download release from
    // https://github.com/knative/client/releases.
    depends_on_one_of: &["kubectl"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kn_registration_shape() {
        assert_eq!(KN.command, "kn");
        let mac = KN.macos.expect("kn must support macOS");
        assert_eq!(mac.brew, Some("kn"));
    }
}
