//! hadolint - Dockerfile linter
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(HADOLINT, {
    command: "hadolint",
    macos: { brew: "hadolint" },
    linux: { brew: "hadolint" },
    windows: { winget: "hadolint.hadolint" },
    bsd: { pkg: "hadolint" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_hadolint_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
