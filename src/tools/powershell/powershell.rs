//! powershell - PowerShell Core (pwsh)
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(POWERSHELL, {
    command: "pwsh",
    macos: { cask: "powershell" },
    linux: { uniform: "powershell" },
    windows: { winget: "Microsoft.PowerShell" },
    bsd: { pkg: "powershell" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_powershell_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
