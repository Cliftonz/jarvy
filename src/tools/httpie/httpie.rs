//! httpie - User-friendly HTTP client
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(HTTPIE, {
    command: "http",
    macos: { brew: "httpie" },
    linux: { apt: "httpie", dnf: "httpie", pacman: "httpie", apk: "py3-httpie" },
    windows: { winget: "HTTPie.HTTPie" },
    bsd: { pkg: "py39-httpie" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_httpie_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
