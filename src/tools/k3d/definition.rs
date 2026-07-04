//! k3d - k3s in Docker
//!
//! k3d is a lightweight wrapper to run k3s (Rancher Lab's minimal Kubernetes distribution) in Docker.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(K3D, {
    command: "k3d",
    repo: "k3d-io/k3d",
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
    fn k3d_registration_shape() {
        assert_eq!(K3D.command, "k3d");
        let mac = K3D.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("k3d"));
        let win = K3D.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("k3d-io.k3d"));
    }
}
