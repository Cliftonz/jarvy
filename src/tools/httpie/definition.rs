//! httpie - User-friendly HTTP client
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(HTTPIE, {
    command: "http",
    repo: "httpie/cli",
    macos: { brew: "httpie" },
    linux: { apt: "httpie", dnf: "httpie", pacman: "httpie", apk: "py3-httpie" },
    windows: { winget: "HTTPie.HTTPie" },
    bsd: { pkg: "py39-httpie" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn httpie_registration_shape() {
        assert_eq!(HTTPIE.command, "http");
        let mac = HTTPIE.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("httpie"));
        let win = HTTPIE.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("HTTPie.HTTPie"));
    }
}
