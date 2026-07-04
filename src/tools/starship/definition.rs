//! starship - minimal, fast, customizable shell prompt
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(STARSHIP, {
    command: "starship",
    repo: "starship/starship",
    macos: { brew: "starship" },
    linux: { uniform: "starship" },
    windows: { winget: "Starship.Starship" },
    bsd: { pkg: "starship" },
    default_hook_shell_init: ("starship", "init"),
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starship_registration_shape() {
        assert_eq!(STARSHIP.command, "starship");
        let mac = STARSHIP.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("starship"));
        let win = STARSHIP.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Starship.Starship"));
    }
}
