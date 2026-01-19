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
    fn ensure_ruby_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
