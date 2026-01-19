//! shfmt - Shell script formatter
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(SHFMT, {
    command: "shfmt",
    macos: { brew: "shfmt" },
    linux: { uniform: "shfmt" },
    windows: { winget: "mvdan.shfmt" },
    bsd: { pkg: "shfmt" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_shfmt_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
