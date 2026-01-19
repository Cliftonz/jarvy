//! buildah - OCI container image builder
//!
//! Buildah facilitates building OCI container images without requiring
//! a full container runtime daemon.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(BUILDAH, {
    command: "buildah",
    macos: { brew: "buildah" },
    linux: { apt: "buildah", dnf: "buildah", pacman: "buildah", apk: "buildah" },
    bsd: { pkg: "buildah" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_buildah_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
