//! yq - YAML/JSON/XML processor (mikefarah/yq)
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(YQ, {
    command: "yq",
    macos: { brew: "yq" },
    linux: { uniform: "yq" },
    windows: { winget: "mikefarah.yq" },
    bsd: { pkg: "yq" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_yq_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
