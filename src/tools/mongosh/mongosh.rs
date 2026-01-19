//! mongosh - MongoDB Shell
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(MONGOSH, {
    command: "mongosh",
    macos: { brew: "mongosh" },
    linux: { brew: "mongosh" },
    windows: { winget: "MongoDB.Shell" },
    bsd: { pkg: "mongosh" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_mongosh_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
