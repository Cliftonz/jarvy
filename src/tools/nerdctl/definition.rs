//! nerdctl - containerd CLI
//!
//! nerdctl is a Docker-compatible CLI for containerd.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(NERDCTL, {
    command: "nerdctl",
    macos: { brew: "nerdctl" },
    linux: { brew: "nerdctl", apk: "nerdctl" },
    bsd: { pkg: "nerdctl" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nerdctl_registration_shape() {
        assert_eq!(NERDCTL.command, "nerdctl");
        let mac = NERDCTL.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("nerdctl"));
    }
}
