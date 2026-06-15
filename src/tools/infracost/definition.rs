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
    fn infracost_registration_shape() {
        assert_eq!(INFRACOST.command, "infracost");
        let mac = INFRACOST.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("infracost"));
        let win = INFRACOST.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Infracost.Infracost"));
    }
}
