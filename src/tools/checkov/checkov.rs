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
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_checkov_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
