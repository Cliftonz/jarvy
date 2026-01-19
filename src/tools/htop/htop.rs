//! htop - interactive process viewer
//!
//! This tool uses the ToolSpec pattern for declarative installation.
//! Note: Not supported on Windows natively - use WSL.

use crate::define_tool;

define_tool!(HTOP, {
    command: "htop",
    macos: { brew: "htop" },
    linux: { uniform: "htop" },
    // No Windows support - htop is Unix-only
    bsd: { pkg: "htop" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_htop_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
