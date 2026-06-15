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
    fn vim_registration_shape() {
        assert_eq!(VIM.command, "vim");
        let mac = VIM.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("vim"));
        let win = VIM.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("vim.vim"));
    }
}
