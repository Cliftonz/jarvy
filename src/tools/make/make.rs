//! make - build automation tool
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(MAKE, {
    command: "make",
    macos: { brew: "make" },
    linux: { uniform: "make" },
    windows: { winget: "GnuWin32.Make" },
    bsd: { pkg: "gmake" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_make_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
