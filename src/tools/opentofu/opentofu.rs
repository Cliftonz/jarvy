//! opentofu - open-source Terraform alternative
//!
//! This tool uses the ToolSpec pattern for declarative installation.
//! Note: The command is `tofu`, not `opentofu`.

use crate::define_tool;

define_tool!(OPENTOFU, {
    command: "tofu",
    macos: { brew: "opentofu" },
    linux: { uniform: "opentofu" },
    windows: { winget: "OpenTofu.OpenTofu" },
    bsd: { pkg: "opentofu" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_opentofu_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
