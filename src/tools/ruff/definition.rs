//! ruff - Extremely fast Python linter and formatter
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(RUFF, {
    command: "ruff",
    macos: { brew: "ruff" },
    linux: { brew: "ruff", apk: "ruff" },
    windows: { winget: "astral-sh.ruff" },
    bsd: { pkg: "ruff" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ruff_registration_shape() {
        assert_eq!(RUFF.command, "ruff");
        let mac = RUFF.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("ruff"));
        let win = RUFF.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("astral-sh.ruff"));
    }
}
