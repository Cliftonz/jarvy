//! gitleaks - secret scanning tool
//!
//! Gitleaks is a SAST tool for detecting and preventing hardcoded secrets.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(GITLEAKS, {
    command: "gitleaks",
    macos: { brew: "gitleaks" },
    linux: { brew: "gitleaks" },
    windows: { winget: "Gitleaks.Gitleaks" },
    bsd: { pkg: "gitleaks" },
    depends_on: &["git"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_gitleaks_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
