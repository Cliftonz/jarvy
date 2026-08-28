//! 1Password CLI — secrets/vault access from services, CI, and shell.
//!
//! Homepage: <https://developer.1password.com/docs/cli/>. Binary is `op`.
//!
//! ## Package coverage
//!
//! Verified against <https://developer.1password.com/docs/cli/get-started/>:
//!
//! - macOS: `brew install 1password-cli`.
//! - Windows: `winget install 1password-cli`. No Chocolatey or Scoop
//!   package is documented, so those slots are omitted.
//! - Linux: apt / dnf / apk packages are documented but each requires
//!   1Password's own signed repository/key to be added first (apt: import
//!   a keyring + add a sources.list.d entry; dnf: add a repo file; apk:
//!   add the key + repo). Jarvy's `linux.apt` / `linux.dnf` blocks assume
//!   the package is directly resolvable from the base repos, so — same
//!   situation as `infisical` — we OMIT them and let the runtime emit
//!   `tool.unsupported` with a link to the docs rather than silently
//!   mis-installing a name collision from the base repos.

use crate::define_tool;

define_tool!(ONEPASSWORD_CLI, {
    command: "op",
    macos: { brew: "1password-cli" },
    windows: { winget: "1password-cli" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn onepassword_cli_registration_shape() {
        assert_eq!(ONEPASSWORD_CLI.command, "op");
        let mac = ONEPASSWORD_CLI.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("1password-cli"));
        let win = ONEPASSWORD_CLI.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("1password-cli"));
        assert!(
            ONEPASSWORD_CLI.linux.is_none(),
            "linux install requires a vendor repo add step \
             — see the module docs. Omit rather than mis-resolve."
        );
    }
}
