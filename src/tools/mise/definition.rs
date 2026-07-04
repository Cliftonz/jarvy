//! mise - dev tools, env vars, task runner
//!
//! mise (formerly rtx) is a polyglot tool version manager.
//! It manages languages like Node, Python, Ruby, etc.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(MISE, {
    command: "mise",
    repo: "jdx/mise",
    macos: { brew: "mise" },
    linux: { brew: "mise" },
    windows: { winget: "jdx.mise" },
    bsd: { pkg: "mise" },
    default_hook_shell_init: ("mise", "activate"),
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mise_registration_shape() {
        assert_eq!(MISE.command, "mise");
        let mac = MISE.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("mise"));
        let win = MISE.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("jdx.mise"));
    }
}
