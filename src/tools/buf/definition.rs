//! buf - Protobuf tooling for schema management and linting
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(BUF, {
    command: "buf",
    repo: "bufbuild/buf",
    macos: { brew: "bufbuild/buf/buf" },
    linux: { uniform: "buf" },
    windows: { winget: "Bufbuild.Buf" },
    bsd: { pkg: "buf" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buf_registration_shape() {
        assert_eq!(BUF.command, "buf");
        let mac = BUF.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("bufbuild/buf/buf"));
        let win = BUF.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Bufbuild.Buf"));
    }
}
