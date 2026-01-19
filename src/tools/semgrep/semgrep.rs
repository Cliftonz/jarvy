//! semgrep - semantic code analysis
//!
//! Semgrep is a fast, open-source static analysis tool for finding bugs
//! and enforcing code standards.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(SEMGREP, {
    command: "semgrep",
    macos: { brew: "semgrep" },
    linux: { brew: "semgrep" },
    bsd: { pkg: "semgrep" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_semgrep_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
