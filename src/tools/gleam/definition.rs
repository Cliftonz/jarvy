//! gleam - Gleam programming language
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(GLEAM, {
    command: "gleam",
    repo: "gleam-lang/gleam",
    macos: { brew: "gleam" },
    linux: { uniform: "gleam" },
    windows: { winget: "Gleam.Gleam" },
    bsd: { pkg: "gleam" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gleam_registration_shape() {
        assert_eq!(GLEAM.command, "gleam");
        let mac = GLEAM.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("gleam"));
        let win = GLEAM.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Gleam.Gleam"));
    }
}
