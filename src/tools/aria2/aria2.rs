//! aria2 - lightweight multi-protocol download utility
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(ARIA2, {
    command: "aria2c",
    macos: { brew: "aria2" },
    linux: { uniform: "aria2" },
    windows: { winget: "aria2.aria2" },
    bsd: { pkg: "aria2" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_aria2_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
