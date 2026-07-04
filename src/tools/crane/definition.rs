//! crane - container registry tool
//!
//! Crane is a tool for interacting with remote container images and registries.
//! It can copy, list, mutate, and inspect container images without a Docker daemon.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(CRANE, {
    command: "crane",
    repo: "google/go-containerregistry",
    macos: { brew: "crane" },
    linux: { uniform: "crane" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crane_registration_shape() {
        assert_eq!(CRANE.command, "crane");
        let mac = CRANE.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("crane"));
    }
}
