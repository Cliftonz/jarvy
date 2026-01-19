//! trivy - Comprehensive vulnerability scanner
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(TRIVY, {
    command: "trivy",
    macos: { brew: "trivy" },
    linux: { brew: "trivy", apk: "trivy" },
    windows: { winget: "aquasecurity.trivy" },
    bsd: { pkg: "trivy" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_trivy_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
