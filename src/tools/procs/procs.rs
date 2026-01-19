//! procs - Modern replacement for ps
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(PROCS, {
    command: "procs",
    macos: { brew: "procs" },
    linux: { uniform: "procs" },
    windows: { winget: "dalance.procs" },
    bsd: { pkg: "procs" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_procs_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
