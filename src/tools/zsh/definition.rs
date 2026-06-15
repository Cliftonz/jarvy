//! zsh - Z shell
//!
//! This tool uses the ToolSpec pattern for declarative installation.
//! Note: Not supported on Windows natively - use WSL or MSYS2.

use crate::define_tool;

define_tool!(ZSH, {
    command: "zsh",
    macos: { brew: "zsh" },
    linux: { uniform: "zsh" },
    // No Windows support - zsh is Unix-only
    bsd: { pkg: "zsh" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zsh_registration_shape() {
        assert_eq!(ZSH.command, "zsh");
        let mac = ZSH.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("zsh"));
    }
}
