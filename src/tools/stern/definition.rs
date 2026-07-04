//! stern - Multi-pod log tailing for Kubernetes
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(STERN, {
    command: "stern",
    repo: "stern/stern",
    macos: { brew: "stern" },
    linux: { uniform: "stern" },
    windows: { winget: "stern.stern" },
    bsd: { pkg: "stern" },
    depends_on: &["kubectl"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stern_registration_shape() {
        assert_eq!(STERN.command, "stern");
        let mac = STERN.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("stern"));
        let win = STERN.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("stern.stern"));
    }
}
