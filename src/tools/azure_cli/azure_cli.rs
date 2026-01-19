//! azure-cli - Microsoft Azure command-line interface
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(AZURE_CLI, {
    command: "az",
    macos: { brew: "azure-cli" },
    linux: { apt: "azure-cli", dnf: "azure-cli", pacman: "azure-cli", apk: "azure-cli" },
    windows: { winget: "Microsoft.AzureCLI" },
    bsd: { pkg: "py39-azure-cli" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_azure_cli_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
