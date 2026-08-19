//! git-town — high-level git workflow CLI (hack / sync / ship).
//!
//! No first-party winget manifest as of 2026-08; upstream install docs
//! list Chocolatey + Scoop for Windows. Module is `git_town`; the
//! registry's dash/underscore aliasing resolves `git-town = "latest"` in
//! jarvy.toml too.

use crate::define_tool;

define_tool!(GIT_TOWN, {
    command: "git-town",
    macos: { brew: "git-town" },
    linux: { brew: "git-town" },
    windows: { choco: "git-town" },
    depends_on: &["git"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_town_registration_shape() {
        assert_eq!(GIT_TOWN.command, "git-town");
        assert_eq!(GIT_TOWN.macos.expect("macOS").brew, Some("git-town"));
        assert_eq!(GIT_TOWN.linux.expect("Linux").brew, Some("git-town"));
        assert_eq!(GIT_TOWN.windows.expect("Windows").choco, Some("git-town"));
        assert!(GIT_TOWN.depends_on.expect("declares deps").contains(&"git"));
    }
}
