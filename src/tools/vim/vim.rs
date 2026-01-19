//! vim - highly configurable text editor
//!
//! Vim is a highly configurable text editor built to make creating and changing text efficient.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(VIM, {
    command: "vim",
    macos: { brew: "vim" },
    linux: { apt: "vim", dnf: "vim-enhanced", pacman: "vim", apk: "vim" },
    windows: { winget: "vim.vim" },
    bsd: { pkg: "vim" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_vim_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
