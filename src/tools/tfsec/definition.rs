//! tfsec - Terraform security scanner
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(TFSEC, {
    command: "tfsec",
    repo: "aquasecurity/tfsec",
    macos: { brew: "tfsec" },
    linux: { uniform: "tfsec" },
    windows: { winget: "aquasecurity.tfsec" },
    bsd: { pkg: "tfsec" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tfsec_registration_shape() {
        assert_eq!(TFSEC.command, "tfsec");
        let mac = TFSEC.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("tfsec"));
        let win = TFSEC.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("aquasecurity.tfsec"));
    }
}
