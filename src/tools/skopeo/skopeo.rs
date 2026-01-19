//! skopeo - container image operations
//!
//! Skopeo performs operations on container images and image repositories.
//! It can copy, inspect, delete, and sign container images.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(SKOPEO, {
    command: "skopeo",
    macos: { brew: "skopeo" },
    linux: { apt: "skopeo", dnf: "skopeo", pacman: "skopeo", apk: "skopeo" },
    bsd: { pkg: "skopeo" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_skopeo_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
