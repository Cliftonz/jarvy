//! checkov - infrastructure as code scanner
//!
//! Checkov scans cloud infrastructure configurations to find misconfigurations
//! before they're deployed.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(CHECKOV, {
    command: "checkov",
    macos: { brew: "checkov" },
    linux: { brew: "checkov" },
    bsd: { pkg: "py39-checkov" },
    // No first-party winget manifest; the uv fallback route covers
    // Windows via PyPI `checkov` (verified 2026-08).
    fallback: { uv: "checkov" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkov_registration_shape() {
        assert_eq!(CHECKOV.command, "checkov");
        let mac = CHECKOV.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("checkov"));
        assert!(CHECKOV.windows.is_none(), "no first-party winget manifest");
        assert_eq!(CHECKOV.fallback.len(), 1);
        assert_eq!(CHECKOV.fallback[0].eco, crate::tools::spec::FallbackEco::Uv);
        assert_eq!(CHECKOV.fallback[0].package, "checkov");
    }
}
