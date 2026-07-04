//! Infisical CLI — secret management for services and local dev.
//!
//! Homepage: <https://infisical.com>. Wraps the Infisical API for
//! fetching, injecting, and rotating secrets from CI + shell (`infisical
//! run -- <cmd>` injects secrets into subprocess env; `infisical
//! secrets` for CRUD).
//!
//! ## Package coverage
//!
//! Verified 2026-07 against <https://infisical.com/docs/cli/overview>:
//!
//! - macOS: `brew install infisical` (homebrew-core formula, i.e. the
//!   canonical Homebrew namespace — not the outdated
//!   `infisical/get-cli/infisical` tap the docs still mention).
//! - Windows: `winget install infisical.infisical`. Verified against
//!   `microsoft/winget-pkgs` — publisher namespace `infisical` is
//!   claimed by Infisical Inc, so the ID is safe.
//! - Linux: apt + yum + apk are documented but each requires a
//!   Microsoft-hosted repo to be added first (Infisical does not ship
//!   into standard debian / RHEL / Alpine repos). Jarvy's `linux.apt`
//!   / `linux.dnf` blocks assume the package is directly resolvable,
//!   so we OMIT them and let the runtime emit `tool.unsupported` with
//!   a link to the docs rather than silently mis-installing a name
//!   collision from the base repos.

use crate::define_tool;

define_tool!(INFISICAL, {
    command: "infisical",
    repo: "Infisical/infisical",
    macos: { brew: "infisical" },
    windows: { winget: "infisical.infisical" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infisical_registration_shape() {
        assert_eq!(INFISICAL.command, "infisical");
        let mac = INFISICAL.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("infisical"));
        let win = INFISICAL.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("infisical.infisical"));
        assert!(
            INFISICAL.linux.is_none(),
            "linux install requires a Microsoft-hosted repo add step \
             — see the module docs. Omit rather than mis-resolve."
        );
    }
}
