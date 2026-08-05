//! govulncheck - Go vulnerability checker
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(GOVULNCHECK, {
    command: "govulncheck",
    macos: { brew: "govulncheck" },
    linux: { uniform: "govulncheck" },
    bsd: { pkg: "govulncheck" },
    // No first-party winget manifest; the go fallback route covers
    // Windows (verified 2026-08).
    fallback: { go: "golang.org/x/vuln/cmd/govulncheck" },
    depends_on: &["go"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn govulncheck_registration_shape() {
        assert_eq!(GOVULNCHECK.command, "govulncheck");
        let mac = GOVULNCHECK.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("govulncheck"));
        assert!(
            GOVULNCHECK.windows.is_none(),
            "no first-party winget manifest"
        );
        assert_eq!(GOVULNCHECK.fallback.len(), 1);
        assert_eq!(
            GOVULNCHECK.fallback[0].eco,
            crate::tools::spec::FallbackEco::Go
        );
        assert_eq!(
            GOVULNCHECK.fallback[0].package,
            "golang.org/x/vuln/cmd/govulncheck"
        );
    }
}
