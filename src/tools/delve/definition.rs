//! delve - Go debugger
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(DELVE, {
    command: "dlv",
    repo: "go-delve/delve",
    macos: { brew: "delve" },
    linux: { uniform: "delve" },
    windows: { winget: "Go.Delve" },
    bsd: { pkg: "delve" },
    depends_on: &["go"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delve_registration_shape() {
        assert_eq!(DELVE.command, "dlv");
        let mac = DELVE.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("delve"));
        let win = DELVE.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Go.Delve"));
    }
}
