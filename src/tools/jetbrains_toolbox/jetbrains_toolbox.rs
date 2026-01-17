//! jetbrains-toolbox - JetBrains IDE management tool
//!
//! JetBrains Toolbox App manages all JetBrains IDEs and keeps them up to date.
//! Note: Linux install via official tarball from jetbrains.com/toolbox-app
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(JETBRAINS_TOOLBOX, {
    command: "jetbrains-toolbox",
    macos: { cask: "jetbrains-toolbox" },
    windows: { winget: "JetBrains.Toolbox" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_jetbrains_toolbox_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
