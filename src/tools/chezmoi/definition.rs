//! chezmoi — cross-platform dotfile manager.
//!
//! Used by the `[dotfiles]` block to fetch and apply a personal dotfile
//! repository during `jarvy setup`, so `nvim`, `tmux`, shell rc files,
//! etc. sync across machines. `manager = "chezmoi"` in `jarvy.toml`
//! triggers `chezmoi init --apply <repo>` idempotently.

use crate::define_tool;

define_tool!(CHEZMOI, {
    command: "chezmoi",
    macos: { brew: "chezmoi" },
    linux: { apt: "chezmoi", dnf: "chezmoi", pacman: "chezmoi", apk: "chezmoi" },
    // Verified winget publisher: twpayne is the chezmoi author.
    windows: { winget: "twpayne.chezmoi" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chezmoi_registration_shape() {
        assert_eq!(CHEZMOI.command, "chezmoi");
        let mac = CHEZMOI.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("chezmoi"));
    }
}
