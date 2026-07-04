//! rabbitmq-server - RabbitMQ AMQP/MQTT/STOMP broker
//!
//! The reference AMQP 0-9-1 broker (also speaks MQTT, STOMP, Stream
//! protocol). Ships with `rabbitmqctl` for cluster admin and
//! `rabbitmqadmin` for the HTTP API. The brew formula installs all
//! three; on Linux the distro packages do the same.

use crate::define_tool;

define_tool!(RABBITMQ, {
    command: "rabbitmq-server",
    repo: "rabbitmq/rabbitmq-server",
    macos: { brew: "rabbitmq" },
    linux: { uniform: "rabbitmq-server" },
    // No first-party winget manifest as of 2026-06; the prior
    // `Pivotal.RabbitMQ` id never existed (the Pivotal publisher
    // namespace is unclaimed). Windows users: install from
    // https://www.rabbitmq.com/install-windows.html.
    category: "messaging",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rabbitmq_registration_shape() {
        assert_eq!(RABBITMQ.command, "rabbitmq-server");
        assert_eq!(RABBITMQ.category, Some("messaging"));
        let mac = RABBITMQ.macos.expect("rabbitmq must support macOS");
        assert_eq!(mac.brew, Some("rabbitmq"));
        let linux = RABBITMQ.linux.expect("rabbitmq must support Linux");
        assert_eq!(linux.apt, Some("rabbitmq-server"));
        assert!(RABBITMQ.windows.is_none(), "no first-party winget manifest");
    }
}
