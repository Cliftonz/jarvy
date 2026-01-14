//! just - Command runner (make alternative)
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(JUST, {
    command: "just",
    macos: { brew: "just" },
    linux: { uniform: "just" },
    windows: { winget: "Casey.Just" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_just_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
