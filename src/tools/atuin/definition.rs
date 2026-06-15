//! atuin - magical shell history
//!
//! Atuin replaces your existing shell history with a SQLite database,
//! and records additional context for your commands.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(ATUIN, {
    command: "atuin",
    macos: { brew: "atuin" },
    linux: { uniform: "atuin" },
    windows: { winget: "atuinsh.atuin" },
    bsd: { pkg: "atuin" },
    default_hook_shell_init: ("atuin", "init"),
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atuin_registration_shape() {
        assert_eq!(ATUIN.command, "atuin");
        let mac = ATUIN.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("atuin"));
        let win = ATUIN.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("atuinsh.atuin"));
    }
}
