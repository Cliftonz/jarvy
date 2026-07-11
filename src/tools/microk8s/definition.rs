//! microk8s - Canonical's low-ops, minimal-production Kubernetes
//!
//! MicroK8s runs a full Kubernetes in a single snap (Linux) or inside a
//! Multipass VM driven by the official Homebrew tap formula (macOS).
//! No first-party winget/choco manifest as of 2026-07; on Windows install
//! from https://microk8s.io/docs/install-windows.
//!
//! This tool uses the ToolSpec pattern with a custom installer because the
//! Linux distribution channel is snap-only, which the declarative platform
//! slots don't model.

use crate::define_tool;
use crate::tools::common::{InstallError, Os, current_os, has, run, run_maybe_sudo};

define_tool!(MICROK8S, {
    command: "microk8s",
    custom_install: install_microk8s,
});

fn install_microk8s(_min_hint: &str) -> Result<(), InstallError> {
    if has("microk8s") {
        return Ok(());
    }

    match current_os() {
        Os::Macos => {
            if !has("brew") {
                return Err(InstallError::Prereq(
                    "Homebrew not found. Install https://brew.sh and re-run.",
                ));
            }
            // Official Canonical tap (github.com/ubuntu/homebrew-microk8s).
            // Soft-fail the tap like ToolSpec::install_macos: already-added
            // tap or network trouble surfaces via the install below.
            let _ = run("brew", &["tap", "ubuntu/microk8s"]);
            run("brew", &["install", "ubuntu/microk8s/microk8s"])?;
            Ok(())
        }
        Os::Linux => {
            if !has("snap") {
                return Err(InstallError::Prereq(
                    "snapd is required to install microk8s (https://snapcraft.io/docs/installing-snapd)",
                ));
            }
            let args = ["install", "microk8s", "--classic"];
            // Same sudo autodetect as PkgOps::install: try plain, escalate
            // only when sudo is available.
            if let Err(e) = run_maybe_sudo(false, "snap", &args) {
                if has("sudo") {
                    run_maybe_sudo(true, "snap", &args)?;
                } else {
                    return Err(e);
                }
            }
            Ok(())
        }
        _ => Err(InstallError::Unsupported),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn microk8s_registration_shape() {
        assert_eq!(MICROK8S.command, "microk8s");
        assert!(MICROK8S.custom_install.is_some());
        // No platform blocks: install is fully owned by the custom installer.
        assert!(MICROK8S.macos.is_none());
        assert!(MICROK8S.windows.is_none());
    }
}
