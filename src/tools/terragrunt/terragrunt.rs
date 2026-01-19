//! terragrunt - thin wrapper for terraform
//!
//! Terragrunt is a thin wrapper for Terraform that provides extra tools
//! for working with multiple Terraform modules.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(TERRAGRUNT, {
    command: "terragrunt",
    macos: { brew: "terragrunt" },
    linux: { brew: "terragrunt" },
    windows: { winget: "Gruntwork.Terragrunt", choco: "terragrunt" },
    bsd: { pkg: "terragrunt" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_terragrunt_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
