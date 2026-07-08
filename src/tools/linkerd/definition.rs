//! Linkerd - ultralight service mesh CLI for Kubernetes
//!
//! `linkerd` installs, upgrades, and debugs the Linkerd service mesh
//! (mTLS, golden-metrics observability, traffic shifting) on a
//! Kubernetes cluster.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(LINKERD, {
    command: "linkerd",
    macos: { brew: "linkerd" },
    // Linux: no distro package; install via Linuxbrew or the upstream
    // installer script.
    linux: { brew: "linkerd" },
    // No first-party winget manifest as of 2026-07; download release
    // from https://github.com/linkerd/linkerd2/releases.
    // Mesh CLI needs kubectl to talk to a cluster.
    depends_on_one_of: &["kubectl"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linkerd_registration_shape() {
        assert_eq!(LINKERD.command, "linkerd");
        let mac = LINKERD.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("linkerd"));
        let linux = LINKERD.linux.expect("must support Linux");
        assert_eq!(linux.brew, Some("linkerd"));
        assert!(LINKERD.windows.is_none(), "no first-party winget manifest");
        assert_eq!(LINKERD.depends_on_one_of, Some(&["kubectl"][..]));
    }
}
