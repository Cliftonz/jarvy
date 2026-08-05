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
    // No first-party winget manifest; the uv fallback route covers
    // Windows via PyPI `semgrep` (verified 2026-08).
    fallback: { uv: "semgrep" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semgrep_registration_shape() {
        assert_eq!(SEMGREP.command, "semgrep");
        let mac = SEMGREP.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("semgrep"));
        assert!(SEMGREP.windows.is_none(), "no first-party winget manifest");
        assert_eq!(SEMGREP.fallback.len(), 1);
        assert_eq!(SEMGREP.fallback[0].eco, crate::tools::spec::FallbackEco::Uv);
        assert_eq!(SEMGREP.fallback[0].package, "semgrep");
    }
}
