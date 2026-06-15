//! GNU Wget - network file downloader
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(WGET, {
    command: "wget",
    macos: { brew: "wget" },
    linux: { uniform: "wget" },
    windows: { winget: "GnuWin32.Wget" },
    bsd: { pkg: "wget" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wget_registration_shape() {
        assert_eq!(WGET.command, "wget");
        let mac = WGET.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("wget"));
        let win = WGET.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("GnuWin32.Wget"));
    }
}
