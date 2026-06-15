//! yamllint - YAML linter
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(YAMLLINT, {
    command: "yamllint",
    macos: { brew: "yamllint" },
    linux: { apt: "yamllint", dnf: "yamllint", pacman: "yamllint", apk: "py3-yamllint" },
    windows: { winget: "adrienverge.yamllint" },
    bsd: { pkg: "py39-yamllint" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yamllint_registration_shape() {
        assert_eq!(YAMLLINT.command, "yamllint");
        let mac = YAMLLINT.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("yamllint"));
        let win = YAMLLINT.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("adrienverge.yamllint"));
    }
}
