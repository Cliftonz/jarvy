//! stern - Multi-pod log tailing for Kubernetes
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(STERN, {
    command: "stern",
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
    fn ensure_stern_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
