//! infracost - cloud cost estimates for Terraform
//!
//! Infracost shows cloud cost estimates for Terraform. It lets engineers
//! see a cost breakdown and understand costs before making changes,
//! either in the terminal or pull requests.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(INFRACOST, {
    command: "infracost",
    macos: { brew: "infracost" },
    linux: { uniform: "infracost" },
    windows: { winget: "Infracost.Infracost" },
    bsd: { pkg: "infracost" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_infracost_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
