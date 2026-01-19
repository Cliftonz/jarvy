//! dive - Docker image layer explorer
//!
//! A tool for exploring a docker image, layer contents, and discovering ways to shrink the image.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(DIVE, {
    command: "dive",
    macos: { brew: "dive" },
    linux: { brew: "dive", apk: "dive" },
    bsd: { pkg: "dive" },
    // dive can explore images from docker or podman
    depends_on_one_of: &["docker", "podman"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_dive_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
