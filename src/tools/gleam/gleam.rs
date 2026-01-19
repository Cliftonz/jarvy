//! gleam - Gleam programming language
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(GLEAM, {
    command: "gleam",
    macos: { brew: "gleam" },
    linux: { uniform: "gleam" },
    windows: { winget: "Gleam.Gleam" },
    bsd: { pkg: "gleam" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_gleam_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
