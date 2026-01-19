//! hugo - Static site generator
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(HUGO, {
    command: "hugo",
    macos: { brew: "hugo" },
    linux: { uniform: "hugo" },
    windows: { winget: "Hugo.Hugo.Extended" },
    bsd: { pkg: "hugo" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_hugo_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
