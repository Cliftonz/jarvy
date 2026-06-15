//! ruby - Ruby programming language
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(RUBY, {
    command: "ruby",
    macos: { brew: "ruby" },
    linux: { uniform: "ruby" },
    windows: { winget: "RubyInstallerTeam.Ruby" },
    bsd: { pkg: "ruby" },
    // Install rbenv before ruby if both are in the config
    depends_on: &["rbenv"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ruby_registration_shape() {
        assert_eq!(RUBY.command, "ruby");
        let mac = RUBY.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("ruby"));
        let win = RUBY.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("RubyInstallerTeam.Ruby"));
    }
}
