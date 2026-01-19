//! mtr - Network diagnostic tool (traceroute + ping)
//!
//! This tool uses the ToolSpec pattern for declarative installation.
//! Note: No Windows support available.

use crate::define_tool;

define_tool!(MTR, {
    command: "mtr",
    macos: { brew: "mtr" },
    linux: { uniform: "mtr" },
    bsd: { pkg: "mtr" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_mtr_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
