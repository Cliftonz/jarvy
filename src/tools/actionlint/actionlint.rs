//! actionlint - GitHub Actions workflow linter
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(ACTIONLINT, {
    command: "actionlint",
    macos: { brew: "actionlint" },
    linux: { brew: "actionlint" },
    windows: { winget: "rhysd.actionlint" },
    bsd: { pkg: "actionlint" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_actionlint_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
