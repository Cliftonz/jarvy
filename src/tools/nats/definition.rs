//! nats - NATS messaging system CLI
//!
//! The official NATS CLI for publishing, subscribing, managing
//! JetStream streams + consumers, replaying messages, and admin
//! operations against a NATS cluster. Pairs with `nats-server` for
//! local dev and `nsc` for credential management.
//!
//! ## Install paths
//! - macOS / Linux: Homebrew tap `nats-io/nats-tools/nats` (run
//!   `brew tap nats-io/nats-tools` once; jarvy does this implicitly).
//! - Windows: winget id `NATSAuthors.CLI` (verified against
//!   microsoft/winget-pkgs).

use crate::define_tool;

define_tool!(NATS, {
    command: "nats",
    repo: "nats-io/natscli",
    macos: { brew: "nats-io/nats-tools/nats" },
    // Linux: use Linuxbrew via the same tap rather than a distro
    // `natscli` package — verified that no Debian / RHEL family ships
    // a package by that name (security review F3).
    linux: { brew: "nats-io/nats-tools/nats" },
    windows: { winget: "NATSAuthors.CLI" },
    category: "messaging",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nats_registration_shape() {
        assert_eq!(NATS.command, "nats");
        assert_eq!(NATS.category, Some("messaging"));
        let mac = NATS.macos.expect("nats must support macOS");
        assert_eq!(
            mac.brew,
            Some("nats-io/nats-tools/nats"),
            "macOS formula lives in the nats-io/nats-tools tap (verify upstream if this fails — formula may have promoted to homebrew-core)"
        );
        let linux = NATS.linux.expect("nats must support Linux");
        assert_eq!(
            linux.brew,
            Some("nats-io/nats-tools/nats"),
            "Linux install path is Linuxbrew via the nats-io tap"
        );
        let win = NATS.windows.expect("nats must support Windows");
        assert_eq!(
            win.winget,
            Some("NATSAuthors.CLI"),
            "winget id verified against microsoft/winget-pkgs as NATSAuthors.CLI"
        );
    }
}
