//! git-secrets - AWS credential scanner
//!
//! git-secrets by AWS Labs prevents committing secrets and credentials
//! into git repositories by scanning commits, commit messages, and merges.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(GIT_SECRETS, {
    command: "git-secrets",
    repo: "awslabs/git-secrets",
    macos: { brew: "git-secrets" },
    linux: { uniform: "git-secrets" },
    depends_on: &["git"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_secrets_registration_shape() {
        assert_eq!(GIT_SECRETS.command, "git-secrets");
        let mac = GIT_SECRETS.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("git-secrets"));
    }
}
