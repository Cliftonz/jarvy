//! lastpass-cli - LastPass command line vault (upstream: lastpass/lastpass-cli)
//!
//! Package name is `lastpass-cli`; the installed binary is `lpass`. No
//! first-party winget/choco/scoop manifest exists — upstream's README lists
//! supported platforms as GNU/Linux, Cygwin, and Mac OS X only, so the
//! `windows` block is omitted rather than pointed at a package that doesn't
//! exist.

use crate::define_tool;

define_tool!(LASTPASS_CLI, {
    command: "lpass",
    macos: { brew: "lastpass-cli" },
    linux: { uniform: "lastpass-cli" },
    bsd: { pkg: "lastpass-cli" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lastpass_cli_registration_shape() {
        assert_eq!(LASTPASS_CLI.command, "lpass");
        let mac = LASTPASS_CLI.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("lastpass-cli"));
        let linux = LASTPASS_CLI.linux.expect("must support Linux");
        assert_eq!(linux.apt, Some("lastpass-cli"));
        assert_eq!(linux.dnf, Some("lastpass-cli"));
        assert_eq!(linux.yum, Some("lastpass-cli"));
        assert_eq!(linux.pacman, Some("lastpass-cli"));
        let bsd = LASTPASS_CLI.bsd.expect("must support BSD");
        assert_eq!(bsd.pkg, Some("lastpass-cli"));
        assert!(LASTPASS_CLI.windows.is_none());
    }
}
