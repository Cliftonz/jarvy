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
    repo: "knative/client",
    macos: { brew: "kn" },
    // Linux: install via Linuxbrew or the upstream release binary —
    // `kn` is a 2-letter generic name that's namespace-squat-prone if
    // we trust distro apt repositories.
    linux: { brew: "kn" },
    // No first-party winget manifest; download release from
    // https://github.com/knative/client/releases.
    depends_on_one_of: &["kubectl"],
    category: "workflow",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kn_registration_shape() {
        assert_eq!(KN.command, "kn");
        assert_eq!(KN.category, Some("workflow"));
        let mac = KN.macos.expect("kn must support macOS");
        assert_eq!(mac.brew, Some("kn"));
        let linux = KN.linux.expect("kn must support Linux");
        assert_eq!(linux.brew, Some("kn"));
        assert!(KN.windows.is_none(), "no first-party winget manifest");
    }
}
