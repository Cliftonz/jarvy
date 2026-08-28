//! Bitwarden CLI — the command-line vault (`bw`).
//!
//! Verified 2026-08 against <https://bitwarden.com/help/cli/>:
//!
//! - Windows: Chocolatey package `bitwarden-cli` is the only officially
//!   documented Windows package manager for this tool — no winget, no
//!   Scoop, no Homebrew, and no apt package exist upstream (a winget ID
//!   surfaced in an unrelated web search but is not corroborated by
//!   Bitwarden's own docs, so it is deliberately omitted).
//! - macOS/Linux: no native brew formula or apt/dnf package exists
//!   either. The one officially documented cross-platform method besides
//!   a manual binary download is `npm install -g @bitwarden/cli`, so
//!   macOS/Linux route through the npm fallback (PRD-060): with no
//!   `macos`/`linux` block declared, the platform installer returns
//!   `Unsupported` and `ToolSpec::install` falls through to the fallback
//!   route automatically.

use crate::define_tool;

define_tool!(BITWARDEN_CLI, {
    command: "bw",
    windows: { choco: "bitwarden-cli" },
    fallback: { npm: "@bitwarden/cli" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitwarden_cli_registration_shape() {
        assert_eq!(BITWARDEN_CLI.command, "bw");
        let win = BITWARDEN_CLI.windows.expect("must support Windows");
        assert_eq!(win.choco, Some("bitwarden-cli"));
        assert!(
            BITWARDEN_CLI.macos.is_none(),
            "no native Homebrew formula — covered by npm fallback"
        );
        assert!(
            BITWARDEN_CLI.linux.is_none(),
            "no native apt/dnf package — covered by npm fallback"
        );
        assert_eq!(BITWARDEN_CLI.fallback.len(), 1);
        assert_eq!(
            BITWARDEN_CLI.fallback[0].eco,
            crate::tools::spec::FallbackEco::Npm
        );
        assert_eq!(BITWARDEN_CLI.fallback[0].package, "@bitwarden/cli");
    }
}
