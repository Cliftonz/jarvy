//! mockgen - Go mock generator for interfaces
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(MOCKGEN, {
    command: "mockgen",
    macos: { brew: "mockery" },
    linux: { uniform: "mockery" },
    windows: { winget: "vektra.mockery" },
    bsd: { pkg: "mockery" },
    depends_on: &["go"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_mockgen_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
