//! git-lfs - Git Large File Storage
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(GIT_LFS, {
    command: "git-lfs",
    macos: { brew: "git-lfs" },
    linux: { uniform: "git-lfs" },
    windows: { winget: "GitHub.GitLFS" },
    bsd: { pkg: "git-lfs" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_git_lfs_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
