//! jq - command-line JSON processor
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(JQ, {
    command: "jq",
    repo: "jqlang/jq",
    macos: { brew: "jq" },
    linux: { uniform: "jq" },
    windows: { winget: "jqlang.jq" },
    bsd: { pkg: "jq" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jq_registration_shape() {
        assert_eq!(JQ.command, "jq");
        let mac = JQ.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("jq"));
        let win = JQ.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("jqlang.jq"));
    }
}
