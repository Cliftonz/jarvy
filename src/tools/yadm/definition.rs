//! yadm — Yet Another Dotfiles Manager (git-based).
//!
//! Alternative to chezmoi selected via `[dotfiles] manager = "yadm"`.
//! yadm wraps `git` operations against a bare repo in
//! `~/.local/share/yadm/repo.git`, so it's naturally cross-machine.
//!
//! No first-party winget manifest as of 2026-07; yadm on Windows is
//! ecosystem-installed via Cygwin/WSL. Omitting the windows block per
//! the CLAUDE.md rule against placeholder ids.

use crate::define_tool;

define_tool!(YADM, {
    command: "yadm",
    macos: { brew: "yadm" },
    linux: { apt: "yadm", dnf: "yadm", pacman: "yadm", apk: "yadm" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yadm_registration_shape() {
        assert_eq!(YADM.command, "yadm");
        let mac = YADM.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("yadm"));
    }
}
