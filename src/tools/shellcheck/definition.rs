//! shellcheck - Shell script static analysis tool
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(SHELLCHECK, {
    command: "shellcheck",
    repo: "koalaman/shellcheck",
    macos: { brew: "shellcheck" },
    linux: { uniform: "shellcheck" },
    windows: { winget: "koalaman.shellcheck" },
    bsd: { pkg: "hs-ShellCheck" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shellcheck_registration_shape() {
        assert_eq!(SHELLCHECK.command, "shellcheck");
        let mac = SHELLCHECK.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("shellcheck"));
        let win = SHELLCHECK.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("koalaman.shellcheck"));
    }
}
