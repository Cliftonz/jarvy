//! Allure - test report framework with rich visualizations
//!
//! `allure` generates interactive HTML reports from test results
//! produced by JUnit, pytest, Playwright, Cypress, and dozens of
//! other frameworks. Java-based; the brew formula bundles a JRE
//! launcher script.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(ALLURE, {
    command: "allure",
    macos: { brew: "allure" },
    // Linux: no distro package; homebrew-core's `allure` ships an
    // architecture-independent ("all") bottle that Linuxbrew installs.
    linux: { brew: "allure" },
    // No first-party winget manifest as of 2026-07; install via scoop
    // or from https://allurereport.org/docs/v2/install-for-windows/.
    category: "testing",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allure_registration_shape() {
        assert_eq!(ALLURE.command, "allure");
        assert_eq!(ALLURE.category, Some("testing"));
        let mac = ALLURE.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("allure"));
        let linux = ALLURE.linux.expect("must support Linux");
        assert_eq!(linux.brew, Some("allure"));
        assert!(ALLURE.windows.is_none(), "no first-party winget manifest");
    }
}
