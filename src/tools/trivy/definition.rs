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
    fn trivy_registration_shape() {
        assert_eq!(TRIVY.command, "trivy");
        let mac = TRIVY.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("trivy"));
        let win = TRIVY.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("aquasecurity.trivy"));
    }
}
