//! htop - interactive process viewer
//!
//! This tool uses the ToolSpec pattern for declarative installation.
//! Note: Not supported on Windows natively - use WSL.

use crate::define_tool;

define_tool!(HTOP, {
    command: "htop",
    macos: { brew: "htop" },
    linux: { uniform: "htop" },
    // No Windows support - htop is Unix-only
    bsd: { pkg: "htop" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn htop_registration_shape() {
        assert_eq!(HTOP.command, "htop");
        let mac = HTOP.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("htop"));
    }
}
