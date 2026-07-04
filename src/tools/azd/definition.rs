//! azd - Azure Developer CLI
//!
//! Microsoft's higher-level alternative to `az` for end-to-end app
//! provisioning + deployment workflows (`azd init`, `azd provision`,
//! `azd deploy`, `azd up`). Common in modern .NET / Azure shops as the
//! canonical "from zero to running in Azure" CLI for ASP.NET Core +
//! Container Apps + Static Web Apps projects.

use crate::define_tool;

define_tool!(AZD, {
    command: "azd",
    repo: "Azure/azure-dev",
    macos: { brew: "azure-dev" },
    linux: { uniform: "azure-dev" },
    windows: { winget: "Microsoft.Azd" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn azd_registration_shape() {
        assert_eq!(AZD.command, "azd");
        let mac = AZD.macos.expect("azd must support macOS");
        assert_eq!(
            mac.brew,
            Some("azure-dev"),
            "Homebrew formula is `azure-dev`, not `azd`"
        );
        let win = AZD.windows.expect("azd must support Windows");
        assert_eq!(win.winget, Some("Microsoft.Azd"));
    }
}
