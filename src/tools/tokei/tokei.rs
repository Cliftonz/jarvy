//! tokei - count lines of code quickly
//!
//! tokei is a program that displays statistics about your code.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(TOKEI, {
    command: "tokei",
    macos: { brew: "tokei" },
    linux: { apt: "tokei", dnf: "tokei", pacman: "tokei", apk: "tokei" },
    windows: { winget: "XAMPPRocky.tokei", choco: "tokei" },
    bsd: { pkg: "tokei" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_tokei_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
