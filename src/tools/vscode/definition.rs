//! vscode - Visual Studio Code editor
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(VSCODE, {
    command: "code",
    macos: { cask: "visual-studio-code" },
    linux: { uniform: "code" },
    windows: { winget: "Microsoft.VisualStudioCode" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vscode_registration_shape() {
        assert_eq!(VSCODE.command, "code");
        let mac = VSCODE.macos.expect("must support macOS");
        assert_eq!(mac.cask, Some("visual-studio-code"));
        let win = VSCODE.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Microsoft.VisualStudioCode"));
    }
}
