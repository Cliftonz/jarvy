//! sonar-scanner - Launcher to analyze a project with SonarQube
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(SONAR_SCANNER, {
    command: "sonar-scanner",
    repo: "SonarSource/sonar-scanner-cli",
    macos: { brew: "sonar-scanner" },
    linux: { apt: "sonar-scanner", dnf: "sonar-scanner", pacman: "sonar-scanner", apk: "sonar-scanner" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sonar_scanner_registration_shape() {
        assert_eq!(SONAR_SCANNER.command, "sonar-scanner");
        let mac = SONAR_SCANNER.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("sonar-scanner"));
    }
}
