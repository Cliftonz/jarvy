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
    fn terragrunt_registration_shape() {
        assert_eq!(TERRAGRUNT.command, "terragrunt");
        let mac = TERRAGRUNT.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("terragrunt"));
        let win = TERRAGRUNT.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Gruntwork.Terragrunt"));
    }
}
