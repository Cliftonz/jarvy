//! hugo - Static site generator
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(HUGO, {
    command: "hugo",
    macos: { brew: "hugo" },
    linux: { uniform: "hugo" },
    windows: { winget: "Hugo.Hugo.Extended" },
    bsd: { pkg: "hugo" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hugo_registration_shape() {
        assert_eq!(HUGO.command, "hugo");
        let mac = HUGO.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("hugo"));
        let win = HUGO.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Hugo.Hugo.Extended"));
    }
}
