//! shellcheck - Shell script static analysis tool
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(SHELLCHECK, {
    command: "shellcheck",
    macos: { brew: "shellcheck" },
    linux: { uniform: "shellcheck" },
    windows: { winget: "koalaman.shellcheck" },
    bsd: { pkg: "hs-ShellCheck" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_shellcheck_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
