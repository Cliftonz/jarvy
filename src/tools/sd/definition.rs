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
    fn sd_registration_shape() {
        assert_eq!(SD.command, "sd");
        let mac = SD.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("sd"));
    }
}
