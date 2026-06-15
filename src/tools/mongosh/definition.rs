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
    fn mongosh_registration_shape() {
        assert_eq!(MONGOSH.command, "mongosh");
        let mac = MONGOSH.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("mongosh"));
        let win = MONGOSH.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("MongoDB.Shell"));
    }
}
