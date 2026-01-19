//! pre-commit - Framework for managing multi-language pre-commit hooks
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(PRE_COMMIT, {
    command: "pre-commit",
    macos: { brew: "pre-commit" },
    linux: { uniform: "pre-commit" },
    windows: { choco: "pre-commit" },
    bsd: { pkg: "py39-pre-commit" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_pre_commit_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
