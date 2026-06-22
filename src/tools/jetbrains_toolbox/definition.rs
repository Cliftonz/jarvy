//! jetbrains-toolbox - JetBrains IDE management tool
//!
//! JetBrains Toolbox App manages all JetBrains IDEs and keeps them up to date.
//! On macOS we use the Homebrew cask, on Windows winget, on Linux a
//! sha256-pinned tarball from JetBrains' download server.
//!
//! ## Updating the Linux pins
//!
//! JetBrains publishes a `.sha256` companion next to every tarball, so
//! pin refreshes can be scripted:
//!
//! ```bash
//! curl -fsSL 'https://data.services.jetbrains.com/products/releases?code=TBA&latest=true&type=release' \
//!   | jq '.TBA[0].downloads | {linux, linuxARM64}'
//! # → grab the new linux.link, linuxARM64.link, and .build version
//! curl -fsSL <linux.checksumLink>      # → sha for x86_64
//! curl -fsSL <linuxARM64.checksumLink> # → sha for aarch64
//! ```
//!
//! Update URL + sha pairs in the same commit. The tarball lays out as
//! `jetbrains-toolbox-<build>/bin/jetbrains-toolbox`; we extract with
//! `--strip-components=1` so the binary lands at `bin/jetbrains-toolbox`
//! under our install dir.
//!
//! This tool uses the ToolSpec pattern with a custom installer.

use crate::define_tool;
use crate::tools::common::{InstallError, has, run};
#[cfg(target_os = "linux")]
use crate::tools::pinned_binary_installer::TarballAppPin;

#[cfg(target_os = "linux")]
const TOOLBOX_LINUX_X64_URL: &str =
    "https://download.jetbrains.com/toolbox/jetbrains-toolbox-3.5.0.84344.tar.gz";

#[cfg(target_os = "linux")]
const TOOLBOX_LINUX_X64_SHA256: &str =
    "1bbc5baa8ab664a83153424eb4831786e86628bfc024c4f5a675f45a534678ef";

#[cfg(target_os = "linux")]
const TOOLBOX_LINUX_ARM64_URL: &str =
    "https://download.jetbrains.com/toolbox/jetbrains-toolbox-3.5.0.84344-arm64.tar.gz";

#[cfg(target_os = "linux")]
const TOOLBOX_LINUX_ARM64_SHA256: &str =
    "fa418df9a47e0d638f86f46cbbbc032d7b7bb55937e4ff12308d14fb3fc51307";

// No `linux: { ... }` block: jetbrains-toolbox isn't in any Linux package
// manager (the JetBrains-published tarball is the canonical Linux artifact).
// The custom_install fn handles the Linux tarball path directly. Spec
// dispatcher honors custom_install before linux-block lookup, so omission
// here only affects dry-run metadata.
define_tool!(JETBRAINS_TOOLBOX, {
    command: "jetbrains-toolbox",
    macos: { cask: "jetbrains-toolbox" },
    windows: { winget: "JetBrains.Toolbox" },
    custom_install: install_jetbrains_toolbox,
});

fn install_jetbrains_toolbox(_min_hint: &str) -> Result<(), InstallError> {
    if has("jetbrains-toolbox") {
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        if !has("brew") {
            return Err(InstallError::Prereq(
                "Homebrew not found. Install https://brew.sh and re-run.",
            ));
        }
        run("brew", &["install", "--cask", "jetbrains-toolbox"])?;
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    {
        let (url, sha) = {
            #[cfg(target_arch = "x86_64")]
            {
                (TOOLBOX_LINUX_X64_URL, TOOLBOX_LINUX_X64_SHA256)
            }
            #[cfg(target_arch = "aarch64")]
            {
                (TOOLBOX_LINUX_ARM64_URL, TOOLBOX_LINUX_ARM64_SHA256)
            }
            #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
            {
                return Err(InstallError::Unsupported);
            }
        };
        let installer = TarballAppPin {
            name: "jetbrains-toolbox",
            url,
            sha256: sha,
            binary_relpath: "bin/jetbrains-toolbox",
        };
        run("sh", &["-c", &installer.shell_command()])?;
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        if !has("winget") {
            return Err(InstallError::Prereq(
                "winget not found. Install Windows Package Manager, then re-run.",
            ));
        }
        run("winget", &["install", "-e", "--id", "JetBrains.Toolbox"])?;
        return Ok(());
    }

    #[allow(unreachable_code)]
    Err(InstallError::Unsupported)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jetbrains_toolbox_registration_shape() {
        assert_eq!(JETBRAINS_TOOLBOX.command, "jetbrains-toolbox");
        let mac = JETBRAINS_TOOLBOX.macos.expect("must support macOS");
        assert_eq!(mac.cask, Some("jetbrains-toolbox"));
        let win = JETBRAINS_TOOLBOX.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("JetBrains.Toolbox"));
        assert!(JETBRAINS_TOOLBOX.linux.is_none());
        assert!(JETBRAINS_TOOLBOX.custom_install.is_some());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_pins_are_well_formed() {
        for sha in [TOOLBOX_LINUX_X64_SHA256, TOOLBOX_LINUX_ARM64_SHA256] {
            assert_eq!(sha.len(), 64);
            assert!(
                sha.chars()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
            );
        }
        for url in [TOOLBOX_LINUX_X64_URL, TOOLBOX_LINUX_ARM64_URL] {
            assert!(url.starts_with("https://"));
            assert!(!url.contains("/latest/"));
            assert!(url.contains("jetbrains-toolbox-"));
        }
    }
}
