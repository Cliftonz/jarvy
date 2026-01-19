//! duf - Disk usage/free utility with colors
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(DUF, {
    command: "duf",
    macos: { brew: "duf" },
    linux: { uniform: "duf" },
    windows: { winget: "muesli.duf" },
    bsd: { pkg: "duf" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_duf_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
