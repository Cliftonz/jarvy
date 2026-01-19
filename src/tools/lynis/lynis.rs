//! lynis - Security auditing tool for Unix systems
//!
//! This tool uses the ToolSpec pattern for declarative installation.
//! Note: No Windows support available.

use crate::define_tool;

define_tool!(LYNIS, {
    command: "lynis",
    macos: { brew: "lynis" },
    linux: { uniform: "lynis" },
    bsd: { pkg: "lynis" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_lynis_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
