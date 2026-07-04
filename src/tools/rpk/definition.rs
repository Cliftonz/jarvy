//! rpk - Redpanda / Kafka CLI
//!
//! `rpk` is Redpanda's Go-based CLI; it speaks the Kafka wire
//! protocol so it works equally well against Redpanda, Apache
//! Kafka, MSK, Confluent Cloud, and others. Higher-level UX than
//! the official Kafka shell scripts — `rpk topic create`,
//! `rpk cluster info`, `rpk produce`, `rpk consume`, etc. Pair with
//! `kcat` for low-level binary streaming.

use crate::define_tool;

define_tool!(RPK, {
    command: "rpk",
    repo: "redpanda-data/redpanda",
    macos: { brew: "redpanda-data/tap/redpanda" },
    // Linux: install via Linuxbrew (same tap) — the upstream `redpanda`
    // apt package installs the full broker daemon, not just the CLI,
    // and configures systemd units that run as a service. Avoid that
    // surprise; force users through Linuxbrew or the release binary.
    linux: { brew: "redpanda-data/tap/redpanda" },
    // No first-party winget manifest as of 2026-06; the prior
    // `Redpanda.RPK` id was never claimed. Windows users: install
    // from https://github.com/redpanda-data/redpanda/releases.
    category: "messaging",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rpk_registration_shape() {
        assert_eq!(RPK.command, "rpk");
        assert_eq!(RPK.category, Some("messaging"));
        let mac = RPK.macos.expect("rpk must support macOS");
        assert_eq!(
            mac.brew,
            Some("redpanda-data/tap/redpanda"),
            "macOS formula lives in the redpanda-data/tap tap (verify upstream if this fails — formula may have promoted to homebrew-core)"
        );
        let linux = RPK.linux.expect("rpk must support Linux");
        assert_eq!(
            linux.brew,
            Some("redpanda-data/tap/redpanda"),
            "use Linuxbrew, NOT the upstream apt package — that installs the full broker daemon as a system service"
        );
        assert!(RPK.windows.is_none(), "no first-party winget manifest");
    }
}
