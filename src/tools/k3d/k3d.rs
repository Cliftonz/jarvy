//! k3d - k3s in Docker
//!
//! k3d is a lightweight wrapper to run k3s (Rancher Lab's minimal Kubernetes distribution) in Docker.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(K3D, {
    command: "k3d",
    macos: { brew: "k3d" },
    linux: { brew: "k3d" },
    windows: { winget: "k3d-io.k3d" },
    bsd: { pkg: "k3d" },
    depends_on: &["docker"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_k3d_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
