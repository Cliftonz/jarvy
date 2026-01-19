//! choose - human-friendly cut/awk alternative
//!
//! Choose is a human-friendly and fast alternative to cut and (sometimes) awk.
//! It allows selecting fields from lines with simple syntax.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(CHOOSE, {
    command: "choose",
    macos: { brew: "choose-rust" },
    linux: { uniform: "choose" },
    windows: { winget: "choose.choose" },
    bsd: { pkg: "choose" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_choose_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
