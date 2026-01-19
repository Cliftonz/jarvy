//! sd - intuitive find & replace CLI
//!
//! sd is an intuitive find & replace CLI tool, a faster and easier to use alternative to sed.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(SD, {
    command: "sd",
    macos: { brew: "sd" },
    linux: { brew: "sd" },
    windows: { choco: "sd-cli" },
    bsd: { pkg: "sd" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_sd_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
