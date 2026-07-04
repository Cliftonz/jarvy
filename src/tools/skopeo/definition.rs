//! skopeo - container image operations
//!
//! Skopeo performs operations on container images and image repositories.
//! It can copy, inspect, delete, and sign container images.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(SKOPEO, {
    command: "skopeo",
    repo: "podman-container-tools/skopeo",
    macos: { brew: "skopeo" },
    linux: { apt: "skopeo", dnf: "skopeo", pacman: "skopeo", apk: "skopeo" },
    bsd: { pkg: "skopeo" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skopeo_registration_shape() {
        assert_eq!(SKOPEO.command, "skopeo");
        let mac = SKOPEO.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("skopeo"));
    }
}
