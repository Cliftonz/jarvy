//! gofumpt - stricter gofmt
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(GOFUMPT, {
    command: "gofumpt",
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
    fn ensure_gofumpt_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
