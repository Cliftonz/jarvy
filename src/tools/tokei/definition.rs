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
    fn tokei_registration_shape() {
        assert_eq!(TOKEI.command, "tokei");
        let mac = TOKEI.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("tokei"));
        let win = TOKEI.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("XAMPPRocky.tokei"));
    }
}
