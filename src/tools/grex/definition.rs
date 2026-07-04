//! grex - regex generator from user-provided test cases
//!
//! Grex is a command-line tool for generating regular expressions from
//! user-provided test cases. It simplifies regex creation by learning patterns.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(GREX, {
    command: "grex",
    repo: "pemistahl/grex",
    macos: { brew: "grex" },
    linux: { uniform: "grex" },
    windows: { winget: "pemistahl.grex" },
    bsd: { pkg: "grex" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grex_registration_shape() {
        assert_eq!(GREX.command, "grex");
        let mac = GREX.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("grex"));
        let win = GREX.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("pemistahl.grex"));
    }
}
