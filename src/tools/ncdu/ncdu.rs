//! ncdu - NCurses disk usage analyzer
//!
//! This tool uses the ToolSpec pattern for declarative installation.
//! Note: No Windows support available.

use crate::define_tool;

define_tool!(NCDU, {
    command: "ncdu",
    macos: { brew: "ncdu" },
    linux: { uniform: "ncdu" },
    bsd: { pkg: "ncdu" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_ncdu_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
