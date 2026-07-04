//! gofumpt - stricter gofmt
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(GOFUMPT, {
    command: "gofumpt",
    repo: "mvdan/gofumpt",
    macos: { brew: "gofumpt" },
    linux: { uniform: "gofumpt" },
    windows: { winget: "mvdan.gofumpt" },
    bsd: { pkg: "gofumpt" },
    depends_on: &["go"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gofumpt_registration_shape() {
        assert_eq!(GOFUMPT.command, "gofumpt");
        let mac = GOFUMPT.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("gofumpt"));
        let win = GOFUMPT.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("mvdan.gofumpt"));
    }
}
