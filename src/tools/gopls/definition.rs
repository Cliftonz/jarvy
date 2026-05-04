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
    fn ensure_gopls_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
