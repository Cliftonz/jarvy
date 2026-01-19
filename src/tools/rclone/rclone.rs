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
    fn ensure_rclone_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
