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
    fn jetbrains_toolbox_registration_shape() {
        assert_eq!(JETBRAINS_TOOLBOX.command, "jetbrains-toolbox");
        let mac = JETBRAINS_TOOLBOX.macos.expect("must support macOS");
        assert_eq!(mac.cask, Some("jetbrains-toolbox"));
        let win = JETBRAINS_TOOLBOX.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("JetBrains.Toolbox"));
    }
}
