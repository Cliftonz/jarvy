//! GNU stow — symlink-farm-manager for dotfiles.
//!
//! Selected via `[dotfiles] manager = "stow"`. Users maintain a
//! dotfiles repo where each subdirectory represents a package
//! (`nvim/`, `zsh/`, …); `stow <package>` creates symlinks from
//! `$HOME` into that subtree.
//!
//! No first-party winget manifest as of 2026-07; GNU stow on Windows
//! runs via MSYS2/Cygwin. Omitting the windows block per the CLAUDE.md
//! rule against placeholder ids.

use crate::define_tool;

define_tool!(STOW, {
    command: "stow",
    macos: { brew: "stow" },
    linux: { apt: "stow", dnf: "stow", pacman: "stow", apk: "stow" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stow_registration_shape() {
        assert_eq!(STOW.command, "stow");
        let mac = STOW.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("stow"));
    }
}
