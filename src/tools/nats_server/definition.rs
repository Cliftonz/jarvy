//! nats-server - NATS messaging broker
//!
//! The NATS broker itself. Single-binary cloud-native messaging
//! system (Core NATS, JetStream persistence, Key/Value, Object Store).
//! Use this for local development; production typically runs the
//! same binary in a cluster.

use crate::define_tool;

define_tool!(NATS_SERVER, {
    command: "nats-server",
    macos: { brew: "nats-server" },
    linux: { uniform: "nats-server" },
    // No first-party winget manifest as of 2026-06; the prior
    // `Synadia.NATSServer` id never existed in microsoft/winget-pkgs.
    // The go fallback route covers Windows: main package at module
    // root `github.com/nats-io/nats-server/v2` (verified 2026-08, no
    // replace directives in go.mod).
    fallback: { go: "github.com/nats-io/nats-server/v2" },
    category: "messaging",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nats_server_registration_shape() {
        assert_eq!(NATS_SERVER.command, "nats-server");
        assert_eq!(NATS_SERVER.category, Some("messaging"));
        let mac = NATS_SERVER.macos.expect("nats-server must support macOS");
        assert_eq!(mac.brew, Some("nats-server"));
        let linux = NATS_SERVER.linux.expect("nats-server must support Linux");
        assert_eq!(linux.apt, Some("nats-server"));
        assert!(
            NATS_SERVER.windows.is_none(),
            "no first-party winget manifest; install from upstream releases"
        );
        assert_eq!(NATS_SERVER.fallback.len(), 1);
        assert_eq!(
            NATS_SERVER.fallback[0].eco,
            crate::tools::spec::FallbackEco::Go
        );
        assert_eq!(
            NATS_SERVER.fallback[0].package,
            "github.com/nats-io/nats-server/v2"
        );
    }
}
