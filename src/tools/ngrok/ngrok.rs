//! ngrok - secure tunneling to localhost
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(NGROK, {
    command: "ngrok",
    macos: { brew: "ngrok" },
    linux: { uniform: "ngrok" },
    windows: { winget: "Ngrok.Ngrok" },
    bsd: { pkg: "ngrok" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_ngrok_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
