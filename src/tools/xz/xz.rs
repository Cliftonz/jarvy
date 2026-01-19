//! xz - compression utility
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(XZ, {
    command: "xz",
    macos: { brew: "xz" },
    linux: { uniform: "xz" },
    windows: { winget: "XZUtils.XZ" },
    bsd: { pkg: "xz" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_xz_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
