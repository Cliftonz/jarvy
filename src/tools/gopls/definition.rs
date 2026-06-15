//! gopls - Go language server
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(GOPLS, {
    command: "gopls",
    macos: { brew: "gopls" },
    linux: { uniform: "gopls" },
    windows: { winget: "Google.Gopls" },
    bsd: { pkg: "gopls" },
    depends_on: &["go"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gopls_registration_shape() {
        assert_eq!(GOPLS.command, "gopls");
        let mac = GOPLS.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("gopls"));
        let win = GOPLS.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Google.Gopls"));
    }
}
