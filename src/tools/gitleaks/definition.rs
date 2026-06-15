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
    fn gitleaks_registration_shape() {
        assert_eq!(GITLEAKS.command, "gitleaks");
        let mac = GITLEAKS.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("gitleaks"));
        let win = GITLEAKS.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Gitleaks.Gitleaks"));
    }
}
