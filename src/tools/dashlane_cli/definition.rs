//! Dashlane CLI — access Dashlane secrets from the terminal, servers,
//! and CI/CD.
//!
//! Homepage: <https://cli.dashlane.com>. Binary is `dcli` (per the
//! project's own npm `bin` field), not `dashlane` or `dashlane-cli`.
//!
//! Verified 2026-08 against <https://cli.dashlane.com/install>: the
//! only officially documented package-manager install, for BOTH macOS
//! and Linux, is the same Homebrew tap (`brew install
//! dashlane/tap/dashlane-cli`) — a two-slash formula, so jarvy's
//! auto-tap logic in `install_macos`/`install_linux` (see
//! `tools::spec`) handles it on both platforms. Windows has no
//! first-party winget/choco/scoop manifest; the official docs list
//! only a manual packaged-executable download, so the `windows` block
//! is omitted rather than guessing at an unverified third-party ID.

use crate::define_tool;

define_tool!(DASHLANE_CLI, {
    command: "dcli",
    macos: { brew: "dashlane/tap/dashlane-cli" },
    linux: { brew: "dashlane/tap/dashlane-cli" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dashlane_cli_registration_shape() {
        assert_eq!(DASHLANE_CLI.command, "dcli");
        assert_eq!(
            DASHLANE_CLI.macos.expect("macOS").brew,
            Some("dashlane/tap/dashlane-cli")
        );
        assert_eq!(
            DASHLANE_CLI.linux.expect("Linux").brew,
            Some("dashlane/tap/dashlane-cli")
        );
        assert!(
            DASHLANE_CLI.windows.is_none(),
            "no first-party winget/choco/scoop manifest — official docs \
             list only a manual binary download for Windows"
        );
    }
}
