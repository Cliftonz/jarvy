//! semgrep - semantic code analysis
//!
//! Semgrep is a fast, open-source static analysis tool for finding bugs
//! and enforcing code standards.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(SEMGREP, {
    command: "semgrep",
    repo: "semgrep/semgrep",
    macos: { brew: "semgrep" },
    linux: { brew: "semgrep" },
    bsd: { pkg: "semgrep" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semgrep_registration_shape() {
        assert_eq!(SEMGREP.command, "semgrep");
        let mac = SEMGREP.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("semgrep"));
    }
}
