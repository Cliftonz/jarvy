//! mycli - mysql CLI with auto-completion
//!
//! mycli is a command line interface for MySQL with auto-completion
//! and syntax highlighting.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(MYCLI, {
    command: "mycli",
    repo: "dbcli/mycli",
    macos: { brew: "mycli" },
    linux: { apt: "mycli", dnf: "mycli", pacman: "mycli", apk: "mycli" },
    windows: { choco: "mycli" },
    bsd: { pkg: "py39-mycli" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mycli_registration_shape() {
        assert_eq!(MYCLI.command, "mycli");
        let mac = MYCLI.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("mycli"));
    }
}
