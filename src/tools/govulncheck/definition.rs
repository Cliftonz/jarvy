//! govulncheck - Go vulnerability checker
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(GOVULNCHECK, {
    command: "govulncheck",
    macos: { brew: "govulncheck" },
    linux: { uniform: "govulncheck" },
    bsd: { pkg: "govulncheck" },
    depends_on: &["go"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_govulncheck_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
