//! buildah - OCI container image builder
//!
//! Buildah facilitates building OCI container images without requiring
//! a full container runtime daemon.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(BUILDAH, {
    command: "buildah",
    repo: "podman-container-tools/buildah",
    macos: { brew: "buildah" },
    linux: { apt: "buildah", dnf: "buildah", pacman: "buildah", apk: "buildah" },
    bsd: { pkg: "buildah" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buildah_registration_shape() {
        assert_eq!(BUILDAH.command, "buildah");
        let mac = BUILDAH.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("buildah"));
    }
}
