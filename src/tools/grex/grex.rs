//! grex - regex generator from user-provided test cases
//!
//! Grex is a command-line tool for generating regular expressions from
//! user-provided test cases. It simplifies regex creation by learning patterns.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(GREX, {
    command: "grex",
    macos: { brew: "grex" },
    linux: { uniform: "grex" },
    windows: { winget: "pemistahl.grex" },
    bsd: { pkg: "grex" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_grex_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
