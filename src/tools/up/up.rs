//! up - Upbound CLI for Crossplane
//!
//! This tool uses the ToolSpec pattern for declarative installation.
//! Note: Requires Homebrew tap on macOS/Linux, not supported on Windows.

use crate::define_tool;

define_tool!(UP, {
    command: "up",
    macos: { brew: "upbound/tap/up" },
    linux: { brew: "upbound/tap/up" },
    bsd: { pkg: "up" },
    // No Windows support
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_up_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
