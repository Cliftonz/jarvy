//! tilt - local Kubernetes development tool
//!
//! This tool uses the ToolSpec pattern for declarative installation.
//! Note: Not supported on Windows natively.

use crate::define_tool;

define_tool!(TILT, {
    command: "tilt",
    macos: { brew: "tilt" },
    linux: { uniform: "tilt" },
    bsd: { pkg: "tilt" },
    // No Windows support
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_tilt_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
