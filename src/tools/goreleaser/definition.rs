//! goreleaser - Go release automation tool
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(GORELEASER, {
    command: "goreleaser",
    macos: { brew: "goreleaser" },
    linux: { uniform: "goreleaser" },
    windows: { winget: "GoReleaser.GoReleaser" },
    bsd: { pkg: "goreleaser" },
    depends_on: &["go"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_goreleaser_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
