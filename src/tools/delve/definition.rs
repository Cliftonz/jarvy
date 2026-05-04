//! delve - Go debugger
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(DELVE, {
    command: "dlv",
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
    fn ensure_delve_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
