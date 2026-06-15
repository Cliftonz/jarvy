//! actionlint - GitHub Actions workflow linter
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(ACTIONLINT, {
    command: "actionlint",
    macos: { brew: "actionlint" },
    linux: { brew: "actionlint" },
    windows: { winget: "rhysd.actionlint" },
    bsd: { pkg: "actionlint" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actionlint_registration_shape() {
        assert_eq!(ACTIONLINT.command, "actionlint");
        let mac = ACTIONLINT.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("actionlint"));
        let win = ACTIONLINT.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("rhysd.actionlint"));
    }
}
