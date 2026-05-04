//! sonar-scanner - Launcher to analyze a project with SonarQube
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(SONAR_SCANNER, {
    command: "sonar-scanner",
    macos: { brew: "sonar-scanner" },
    linux: { apt: "sonar-scanner", dnf: "sonar-scanner", pacman: "sonar-scanner", apk: "sonar-scanner" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_sonar_scanner_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
