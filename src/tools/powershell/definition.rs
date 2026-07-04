//! powershell - PowerShell Core (pwsh)
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(POWERSHELL, {
    command: "pwsh",
    repo: "PowerShell/PowerShell",
    macos: { cask: "powershell" },
    linux: { uniform: "powershell" },
    windows: { winget: "Microsoft.PowerShell" },
    bsd: { pkg: "powershell" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn powershell_registration_shape() {
        assert_eq!(POWERSHELL.command, "pwsh");
        let mac = POWERSHELL.macos.expect("must support macOS");
        assert_eq!(mac.cask, Some("powershell"));
        let win = POWERSHELL.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Microsoft.PowerShell"));
    }
}
