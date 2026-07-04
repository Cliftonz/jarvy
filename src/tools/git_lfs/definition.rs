//! git-lfs - Git Large File Storage
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(GIT_LFS, {
    command: "git-lfs",
    repo: "git-lfs/git-lfs",
    macos: { brew: "git-lfs" },
    linux: { uniform: "git-lfs" },
    windows: { winget: "GitHub.GitLFS" },
    bsd: { pkg: "git-lfs" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_lfs_registration_shape() {
        assert_eq!(GIT_LFS.command, "git-lfs");
        let mac = GIT_LFS.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("git-lfs"));
        let win = GIT_LFS.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("GitHub.GitLFS"));
    }
}
