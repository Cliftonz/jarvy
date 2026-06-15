//! p7zip - File archiver with high compression (7-Zip)
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(P7ZIP, {
    command: "7z",
    macos: { brew: "p7zip" },
    linux: { uniform: "p7zip" },
    windows: { winget: "7zip.7zip" },
    bsd: { pkg: "p7zip" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p7zip_registration_shape() {
        assert_eq!(P7ZIP.command, "7z");
        let mac = P7ZIP.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("p7zip"));
        let win = P7ZIP.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("7zip.7zip"));
    }
}
