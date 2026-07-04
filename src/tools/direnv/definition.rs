//! direnv - directory-specific environment variables via .envrc
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(DIRENV, {
    command: "direnv",
    repo: "direnv/direnv",
    macos: { brew: "direnv" },
    linux: { uniform: "direnv" },
    windows: { winget: "direnv.direnv" },
    bsd: { pkg: "direnv" },
    default_hook_shell_init: ("direnv", "hook"),
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direnv_registration_shape() {
        assert_eq!(DIRENV.command, "direnv");
        let mac = DIRENV.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("direnv"));
        let win = DIRENV.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("direnv.direnv"));
    }
}
