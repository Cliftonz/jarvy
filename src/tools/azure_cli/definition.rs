//! azure-cli - Microsoft Azure command-line interface
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(AZURE_CLI, {
    command: "az",
    repo: "Azure/azure-cli",
    macos: { brew: "azure-cli" },
    linux: { apt: "azure-cli", dnf: "azure-cli", pacman: "azure-cli", apk: "azure-cli" },
    windows: { winget: "Microsoft.AzureCLI" },
    bsd: { pkg: "py39-azure-cli" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn azure_cli_registration_shape() {
        assert_eq!(AZURE_CLI.command, "az");
        let mac = AZURE_CLI.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("azure-cli"));
        let win = AZURE_CLI.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Microsoft.AzureCLI"));
    }
}
