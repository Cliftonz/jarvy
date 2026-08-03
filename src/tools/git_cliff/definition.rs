//! git-cliff — changelog generator driven by conventional commits.
//!
//! Parses the repo's git history into a templated CHANGELOG, so depends
//! on `git`. Module is `git_cliff`; the registry's dash/underscore
//! aliasing resolves `git-cliff = "latest"` in jarvy.toml too.

use crate::define_tool;

define_tool!(GIT_CLIFF, {
    command: "git-cliff",
    macos: { brew: "git-cliff" },
    linux: { brew: "git-cliff" },
    windows: { winget: "orhun.git-cliff" },
    depends_on: &["git"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_cliff_registration_shape() {
        assert_eq!(GIT_CLIFF.command, "git-cliff");
        assert_eq!(GIT_CLIFF.macos.expect("macOS").brew, Some("git-cliff"));
        assert_eq!(GIT_CLIFF.linux.expect("Linux").brew, Some("git-cliff"));
        assert_eq!(
            GIT_CLIFF.windows.expect("Windows").winget,
            Some("orhun.git-cliff")
        );
        assert!(
            GIT_CLIFF
                .depends_on
                .expect("declares deps")
                .contains(&"git")
        );
    }
}
