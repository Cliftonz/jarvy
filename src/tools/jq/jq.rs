//! jq - command-line JSON processor
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(JQ, {
    command: "jq",
    macos: { brew: "jq" },
    linux: { uniform: "jq" },
    windows: { winget: "jqlang.jq" },
    bsd: { pkg: "jq" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_jq_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
