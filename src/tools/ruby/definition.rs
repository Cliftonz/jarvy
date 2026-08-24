//! ruby - Ruby programming language
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;
use crate::tools::common::Os;

define_tool!(RUBY, {
    command: "ruby",
    macos: { brew: "ruby" },
    linux: { uniform: "ruby" },
    windows: { winget: "RubyInstallerTeam.Ruby" },
    bsd: { pkg: "ruby" },
    // Install rbenv before ruby if both are in the config. rbenv has no
    // Windows route (RubyInstaller is the native path there), so scoping
    // the dep prevents a spurious "rbenv missing" report on Windows.
    depends_on_by_os: &[
        (Os::Macos, "rbenv"),
        (Os::Linux, "rbenv"),
        (Os::Bsd, "rbenv"),
    ],
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

    // rbenv has no Windows route; scope the prereq to the Unix-family
    // OSes so `ruby = "latest"` on Windows doesn't false-report a
    // missing dep.
    #[test]
    fn ruby_scopes_rbenv_dep_to_unix_only() {
        assert!(RUBY.depends_on.is_none());
        assert_eq!(
            RUBY.depends_on_by_os,
            &[
                (Os::Macos, "rbenv"),
                (Os::Linux, "rbenv"),
                (Os::Bsd, "rbenv"),
            ]
        );
    }
}
