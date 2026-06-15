//! nvim - Neovim editor
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(NVIM, {
    command: "nvim",
    macos: { brew: "neovim" },
    linux: { uniform: "neovim" },
    windows: { winget: "Neovim.Neovim" },
    bsd: { pkg: "neovim" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nvim_registration_shape() {
        assert_eq!(NVIM.command, "nvim");
        let mac = NVIM.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("neovim"));
        let win = NVIM.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Neovim.Neovim"));
    }
}
