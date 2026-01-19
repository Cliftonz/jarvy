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
    fn ensure_wget_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
