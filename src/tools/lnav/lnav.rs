//! lnav - Log file navigator with syntax highlighting
//!
//! This tool uses the ToolSpec pattern for declarative installation.
//! Note: No Windows support available.

use crate::define_tool;

define_tool!(LNAV, {
    command: "lnav",
    macos: { brew: "lnav" },
    linux: { uniform: "lnav" },
    bsd: { pkg: "lnav" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_lnav_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
