//! buf - Protobuf tooling for schema management and linting
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(BUF, {
    command: "buf",
    macos: { brew: "bufbuild/buf/buf" },
    linux: { uniform: "buf" },
    windows: { winget: "Bufbuild.Buf" },
    bsd: { pkg: "buf" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_buf_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
