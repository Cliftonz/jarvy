//! curl - Command line tool for transferring data with URLs
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(CURL, {
    command: "curl",
    macos: { brew: "curl" },
    linux: { uniform: "curl" },
    windows: { winget: "cURL.cURL" },
    bsd: { pkg: "curl" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curl_registration_shape() {
        assert_eq!(CURL.command, "curl");
        let mac = CURL.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("curl"));
        let win = CURL.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("cURL.cURL"));
    }
}
