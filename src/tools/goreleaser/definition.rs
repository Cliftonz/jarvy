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
    fn goreleaser_registration_shape() {
        assert_eq!(GORELEASER.command, "goreleaser");
        let mac = GORELEASER.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("goreleaser"));
        let win = GORELEASER.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("GoReleaser.GoReleaser"));
    }
}
