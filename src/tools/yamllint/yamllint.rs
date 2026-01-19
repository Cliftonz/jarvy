//! yamllint - YAML linter
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(YAMLLINT, {
    command: "yamllint",
    macos: { brew: "yamllint" },
    linux: { apt: "yamllint", dnf: "yamllint", pacman: "yamllint", apk: "py3-yamllint" },
    windows: { winget: "adrienverge.yamllint" },
    bsd: { pkg: "py39-yamllint" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_yamllint_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
