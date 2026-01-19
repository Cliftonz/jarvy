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
    fn ensure_p7zip_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
