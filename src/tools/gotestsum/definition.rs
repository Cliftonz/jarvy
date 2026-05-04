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
    fn ensure_gotestsum_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
