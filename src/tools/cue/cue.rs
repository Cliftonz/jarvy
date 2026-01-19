//! cue - CUE configuration language CLI
//!
//! This tool uses the ToolSpec pattern for declarative installation.
//! Note: Not supported on Windows natively.

use crate::define_tool;

define_tool!(CUE, {
    command: "cue",
    macos: { brew: "cue" },
    linux: { uniform: "cue" },
    bsd: { pkg: "cue" },
    // No Windows support
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_cue_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
