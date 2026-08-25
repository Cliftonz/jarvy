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
    // git-lfs is a git subcommand; it will not function without git on
    // any OS. Matches the pattern used by every other git-family tool
    // (git_cliff, git_secrets, git_town, gitleaks, gitversion,
    // trufflehog, talisman, betterleaks) which all declare this.
    // Note: on Windows the GitHub.GitLFS winget manifest already
    // declares PackageDependencies on Git.Git, so winget auto-pulls it;
    // this declaration adds the same ordering guarantee at the jarvy
    // level so macOS/Linux boxes get the ordering too, and jarvy's own
    // dep-check surfaces the requirement when git is absent from
    // config.
    depends_on: &["git"],
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

    // git-lfs is a git subcommand — must declare git as a cross-platform
    // runtime prereq. The rest of the git-family (git_cliff, git_secrets,
    // git_town, gitleaks, gitversion, trufflehog, talisman, betterleaks)
    // all declare this; git_lfs was the sole gap until 2026-08-25.
    #[test]
    fn git_lfs_requires_git() {
        assert_eq!(GIT_LFS.depends_on, Some(&["git"] as &[&str]));
    }
}
