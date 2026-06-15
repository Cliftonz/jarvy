//! zoxide - smarter cd command that learns your navigation patterns
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(ZOXIDE, {
    command: "zoxide",
    macos: { brew: "zoxide" },
    linux: { uniform: "zoxide" },
    windows: { winget: "ajeetdsouza.zoxide" },
    bsd: { pkg: "zoxide" },
    default_hook_shell_init: ("zoxide", "init"),
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoxide_registration_shape() {
        assert_eq!(ZOXIDE.command, "zoxide");
        let mac = ZOXIDE.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("zoxide"));
        let win = ZOXIDE.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("ajeetdsouza.zoxide"));
    }
}
