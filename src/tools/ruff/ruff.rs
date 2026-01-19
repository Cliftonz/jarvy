//! ruff - Extremely fast Python linter and formatter
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(RUFF, {
    command: "ruff",
    macos: { brew: "ruff" },
    linux: { brew: "ruff", apk: "ruff" },
    windows: { winget: "astral-sh.ruff" },
    bsd: { pkg: "ruff" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_ruff_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
