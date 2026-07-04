//! hadolint - Dockerfile linter
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(HADOLINT, {
    command: "hadolint",
    repo: "hadolint/hadolint",
    macos: { brew: "hadolint" },
    linux: { brew: "hadolint" },
    windows: { winget: "hadolint.hadolint" },
    bsd: { pkg: "hadolint" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hadolint_registration_shape() {
        assert_eq!(HADOLINT.command, "hadolint");
        let mac = HADOLINT.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("hadolint"));
        let win = HADOLINT.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("hadolint.hadolint"));
    }
}
