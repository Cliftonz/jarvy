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
    fn yq_registration_shape() {
        assert_eq!(YQ.command, "yq");
        let mac = YQ.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("yq"));
        let win = YQ.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("mikefarah.yq"));
    }
}
