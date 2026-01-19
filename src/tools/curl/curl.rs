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
    fn ensure_curl_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
