//! iterm2 - iTerm2 terminal emulator
//!
//! This tool uses the ToolSpec pattern for declarative installation.
//! Note: macOS only via Homebrew cask.

use crate::define_tool;

define_tool!(ITERM2, {
    command: "iterm2",
    macos: { cask: "iterm2" },
    // No Linux or Windows support - macOS only
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iterm2_registration_shape() {
        assert_eq!(ITERM2.command, "iterm2");
        let mac = ITERM2.macos.expect("must support macOS");
        assert_eq!(mac.cask, Some("iterm2"));
    }
}
