//! air - live reload for Go applications
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(AIR, {
    command: "air",
    macos: { brew: "air" },
    linux: { uniform: "air" },
    windows: { winget: "cosmtrek.air" },
    bsd: { pkg: "air" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn air_registration_shape() {
        assert_eq!(AIR.command, "air");
        let mac = AIR.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("air"));
        let win = AIR.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("cosmtrek.air"));
    }
}
