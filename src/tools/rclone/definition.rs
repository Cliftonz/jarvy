//! rclone - Cloud storage sync and management
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(RCLONE, {
    command: "rclone",
    macos: { brew: "rclone" },
    linux: { uniform: "rclone" },
    windows: { winget: "Rclone.Rclone" },
    bsd: { pkg: "rclone" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rclone_registration_shape() {
        assert_eq!(RCLONE.command, "rclone");
        let mac = RCLONE.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("rclone"));
        let win = RCLONE.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Rclone.Rclone"));
    }
}
