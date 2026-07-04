//! php - PHP programming language
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(PHP, {
    command: "php",
    repo: "php/php-src",
    macos: { brew: "php" },
    linux: { uniform: "php" },
    windows: { winget: "PHP.PHP" },
    bsd: { pkg: "php83" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn php_registration_shape() {
        assert_eq!(PHP.command, "php");
        let mac = PHP.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("php"));
        let win = PHP.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("PHP.PHP"));
    }
}
