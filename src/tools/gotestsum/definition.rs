//! gotestsum - improved Go test runner with formatted output
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(GOTESTSUM, {
    command: "gotestsum",
    macos: { brew: "gotestsum" },
    linux: { uniform: "gotestsum" },
    windows: { winget: "gotestyourself.gotestsum" },
    bsd: { pkg: "gotestsum" },
    depends_on: &["go"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gotestsum_registration_shape() {
        assert_eq!(GOTESTSUM.command, "gotestsum");
        let mac = GOTESTSUM.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("gotestsum"));
        let win = GOTESTSUM.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("gotestyourself.gotestsum"));
    }
}
