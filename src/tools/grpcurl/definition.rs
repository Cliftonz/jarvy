//! grpcurl - command-line tool for interacting with gRPC servers
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(GRPCURL, {
    command: "grpcurl",
    macos: { brew: "grpcurl" },
    linux: { uniform: "grpcurl" },
    windows: { winget: "FullStory.grpcurl" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_grpcurl_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
