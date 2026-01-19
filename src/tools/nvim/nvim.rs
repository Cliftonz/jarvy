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
    fn ensure_nvim_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
