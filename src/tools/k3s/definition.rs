//! k3s - lightweight Kubernetes distribution (Rancher Labs)
//!
//! k3s is a fully conformant, single-binary Kubernetes distribution built
//! for edge, IoT, CI, and development. Linux-only: it installs a systemd /
//! openrc service and the node agent runs directly on the host. For a
//! containerized k3s on macOS/Windows use the `k3d` tool instead.
//!
//! This tool uses the ToolSpec pattern with a custom installer (no distro
//! package; upstream distributes via https://get.k3s.io).

use crate::define_tool;
use crate::tools::common::{InstallError, Os, current_os, has, run};
use crate::tools::pinned_installer::PinnedInstaller;

/// Pinned commit of `k3s-io/k3s` (tag `v1.36.2+k3s1`). Updating this constant
/// is the only way Jarvy will pull a newer k3s installer — no `get.k3s.io`
/// (which tracks the branch tip) fetches at runtime, so a compromise of the
/// upstream tip cannot silently land arbitrary code on the next `jarvy setup`.
///
/// To refresh: pick a release tag's commit, download
/// `https://raw.githubusercontent.com/k3s-io/k3s/<sha>/install.sh`,
/// compute its sha256, update both constants together.
const K3S_INSTALLER_COMMIT: &str = "01b6f04aaa69e8b09303f0393d4b4f1811da23aa";
const K3S_INSTALLER_SHA256: &str =
    "46177d4c99440b4c0311b67233823a8e8a2fc09693f6c89af1a7161e152fbfad";

define_tool!(K3S, {
    command: "k3s",
    custom_install: install_k3s,
});

fn install_k3s(_min_hint: &str) -> Result<(), InstallError> {
    if has("k3s") {
        return Ok(());
    }

    // Linux-only: the installer sets up a systemd/openrc unit on the host.
    if current_os() != Os::Linux {
        return Err(InstallError::Unsupported);
    }

    let url = format!(
        "https://raw.githubusercontent.com/k3s-io/k3s/{}/install.sh",
        K3S_INSTALLER_COMMIT
    );
    let installer = PinnedInstaller {
        name: "k3s",
        url: &url,
        sha256: K3S_INSTALLER_SHA256,
    };
    // The upstream script self-escalates via sudo when not run as root.
    run("sh", &["-c", &installer.shell_command()])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn k3s_registration_shape() {
        assert_eq!(K3S.command, "k3s");
        assert!(K3S.custom_install.is_some());
        // No platform blocks: install is fully owned by the custom installer.
        assert!(K3S.macos.is_none());
        assert!(K3S.windows.is_none());
    }

    #[test]
    fn k3s_installer_pin_shape() {
        assert_eq!(K3S_INSTALLER_COMMIT.len(), 40);
        assert!(K3S_INSTALLER_COMMIT.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(K3S_INSTALLER_SHA256.len(), 64);
        assert!(K3S_INSTALLER_SHA256.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
