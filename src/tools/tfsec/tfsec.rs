//! tfsec - Terraform security scanner
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(TFSEC, {
    command: "tfsec",
    macos: { brew: "tfsec" },
    linux: { uniform: "tfsec" },
    windows: { winget: "aquasecurity.tfsec" },
    bsd: { pkg: "tfsec" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_tfsec_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
