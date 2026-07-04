//! bicep - Microsoft's Azure Resource Manager IaC DSL
//!
//! Bicep compiles to ARM JSON templates. Widely used in .NET / Azure
//! shops as the canonical IaC language for Azure deployments. Often
//! paired with `az deployment` or `azd` in CI/CD pipelines.

use crate::define_tool;

define_tool!(BICEP, {
    command: "bicep",
    repo: "Azure/bicep",
    macos: { brew: "bicep" },
    linux: { uniform: "bicep" },
    windows: { winget: "Microsoft.Bicep" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bicep_registration_shape() {
        assert_eq!(BICEP.command, "bicep");
        let mac = BICEP.macos.expect("bicep must support macOS");
        assert_eq!(mac.brew, Some("bicep"));
        let win = BICEP.windows.expect("bicep must support Windows");
        assert_eq!(win.winget, Some("Microsoft.Bicep"));
    }
}
