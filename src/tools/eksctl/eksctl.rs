//! eksctl - AWS EKS cluster management CLI
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(EKSCTL, {
    command: "eksctl",
    macos: { brew: "eksctl" },
    linux: { uniform: "eksctl" },
    windows: { winget: "weaveworks.eksctl" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_eksctl_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
