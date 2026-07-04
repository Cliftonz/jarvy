//! nsc - NATS account/credential management CLI
//!
//! Tool for managing NATS decentralized auth — operators, accounts,
//! users, signing keys, and credential files. Required when running
//! NATS with the JWT-based auth system (the default for managed NATS
//! cloud and most production clusters).

use crate::define_tool;

define_tool!(NSC, {
    command: "nsc",
    repo: "nats-io/nsc",
    macos: { brew: "nats-io/nats-tools/nsc" },
    linux: { brew: "nats-io/nats-tools/nsc" },
    windows: { winget: "NATSAuthors.nsc" },
    category: "messaging",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nsc_registration_shape() {
        assert_eq!(NSC.command, "nsc");
        assert_eq!(NSC.category, Some("messaging"));
        let mac = NSC.macos.expect("nsc must support macOS");
        assert_eq!(
            mac.brew,
            Some("nats-io/nats-tools/nsc"),
            "macOS formula lives in the nats-io/nats-tools tap (verify upstream if this fails — formula may have promoted to homebrew-core)"
        );
        let linux = NSC.linux.expect("nsc must support Linux");
        assert_eq!(linux.brew, Some("nats-io/nats-tools/nsc"));
        let win = NSC.windows.expect("nsc must support Windows");
        assert_eq!(
            win.winget,
            Some("NATSAuthors.nsc"),
            "winget id verified against microsoft/winget-pkgs as NATSAuthors.nsc"
        );
    }
}
