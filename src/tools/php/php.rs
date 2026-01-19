//! php - PHP programming language
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(PHP, {
    command: "php",
    macos: { brew: "php" },
    linux: { uniform: "php" },
    windows: { winget: "PHP.PHP" },
    bsd: { pkg: "php83" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_php_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
