//! headscale - Open-source, self-hosted Tailscale coordinator
//!
//! Server-side daemon that lets a self-hosted org run a private Tailscale
//! control plane. Distributed as a single static Go binary via GitHub
//! releases — no brew formula, no winget manifest. Linux is the only
//! supported install path (the typical deployment target); macOS and
//! Windows builds exist upstream but the deployment story is Linux-only.
//!
//! ## Updating the Linux pins
//!
//! ```bash
//! TAG=$(curl -fsSL https://api.github.com/repos/juanfont/headscale/releases/latest | jq -r .tag_name)
//! curl -fsSL "https://github.com/juanfont/headscale/releases/download/$TAG/checksums.txt"
//! # → grab linux_amd64 + linux_arm64 sha lines
//! ```
//!
//! Update URL + sha pairs together. Upstream publishes a checksums.txt
//! beside each release so the bump is scriptable.

use crate::define_tool;
#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
use crate::tools::common::run;
use crate::tools::common::{InstallError, has};
#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
use crate::tools::pinned_binary_installer::AppImagePin;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const HEADSCALE_LINUX_X64_URL: &str =
    "https://github.com/juanfont/headscale/releases/download/v0.29.1/headscale_0.29.1_linux_amd64";

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const HEADSCALE_LINUX_X64_SHA256: &str =
    "5d24905749e68ee8ddbb3743f4903959878575aeab59c4a41b61d104853d1888";

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
const HEADSCALE_LINUX_ARM64_URL: &str =
    "https://github.com/juanfont/headscale/releases/download/v0.29.1/headscale_0.29.1_linux_arm64";

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
const HEADSCALE_LINUX_ARM64_SHA256: &str =
    "c1882d7f63e6a7780370bcc20458d9c6c14be72b1743f23f5f950088bd8e24af";

// No platform blocks — the spec dispatcher honors custom_install first,
// and headscale's install paths don't fit any of the macros' shapes
// (raw binary on Linux, server-side-rare on mac/win).
define_tool!(HEADSCALE, {
    command: "headscale",
    custom_install: install_headscale,
    category: "networking",
});

fn install_headscale(_min_hint: &str) -> Result<(), InstallError> {
    if has("headscale") {
        return Ok(());
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        let installer = AppImagePin {
            name: "headscale",
            url: HEADSCALE_LINUX_X64_URL,
            sha256: HEADSCALE_LINUX_X64_SHA256,
        };
        run("sh", &["-c", &installer.shell_command()])?;
        return Ok(());
    }

    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        let installer = AppImagePin {
            name: "headscale",
            url: HEADSCALE_LINUX_ARM64_URL,
            sha256: HEADSCALE_LINUX_ARM64_SHA256,
        };
        run("sh", &["-c", &installer.shell_command()])?;
        return Ok(());
    }

    // Other platforms / arches: build from source or download from
    // upstream releases manually. headscale is a server tool — the
    // typical deployment target is Linux.
    #[allow(unreachable_code)]
    Err(InstallError::Unsupported)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headscale_registration_shape() {
        assert_eq!(HEADSCALE.command, "headscale");
        assert!(HEADSCALE.custom_install.is_some());
        // No platform blocks — install is custom on all targets.
        assert!(HEADSCALE.macos.is_none());
        assert!(HEADSCALE.linux.is_none());
        assert!(HEADSCALE.windows.is_none());
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[test]
    fn linux_x64_pin_is_well_formed() {
        assert_eq!(HEADSCALE_LINUX_X64_SHA256.len(), 64);
        assert!(
            HEADSCALE_LINUX_X64_SHA256
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );
        assert!(HEADSCALE_LINUX_X64_URL.starts_with("https://"));
        assert!(!HEADSCALE_LINUX_X64_URL.contains("/latest/"));
    }

    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    #[test]
    fn linux_arm64_pin_is_well_formed() {
        assert_eq!(HEADSCALE_LINUX_ARM64_SHA256.len(), 64);
        assert!(
            HEADSCALE_LINUX_ARM64_SHA256
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );
        assert!(HEADSCALE_LINUX_ARM64_URL.starts_with("https://"));
        assert!(!HEADSCALE_LINUX_ARM64_URL.contains("/latest/"));
    }
}
