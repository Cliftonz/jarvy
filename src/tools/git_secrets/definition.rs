//! git-secrets - AWS credential scanner
//!
//! git-secrets by AWS Labs prevents committing secrets and credentials
//! into git repositories by scanning commits, commit messages, and merges.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(GIT_SECRETS, {
    command: "git-secrets",
    macos: { brew: "git-secrets" },
    linux: { uniform: "git-secrets" },
    depends_on: &["git"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_git_secrets_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
