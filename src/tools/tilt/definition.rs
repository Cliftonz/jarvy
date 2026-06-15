//! tilt - local Kubernetes development tool
//!
//! This tool uses the ToolSpec pattern for declarative installation.
//! Note: Not supported on Windows natively.

use crate::define_tool;

define_tool!(TILT, {
    command: "tilt",
    macos: { brew: "tilt" },
    linux: { uniform: "tilt" },
    bsd: { pkg: "tilt" },
    // No Windows support
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tilt_registration_shape() {
        assert_eq!(TILT.command, "tilt");
        let mac = TILT.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("tilt"));
    }
}
