//! glab - GitLab CLI
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(GLAB, {
    command: "glab",
    macos: { brew: "glab" },
    linux: { uniform: "glab" },
    windows: { winget: "GLab.GLab" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_glab_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
