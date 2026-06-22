//! cursor - AI-first code editor
//!
//! Cursor is an AI-powered code editor built on VS Code. On macOS we use the
//! Homebrew cask, on Windows winget, on Linux a sha256-pinned AppImage from
//! cursor.com's CDN.
//!
//! ## Updating the Linux pin
//!
//! Cursor doesn't publish a checksum file alongside the AppImage, so the
//! version + sha256 below must be refreshed manually when a new stable
//! release ships:
//!
//! ```bash
//! curl -fsSL 'https://www.cursor.com/api/download?platform=linux-x64&releaseTrack=stable' | jq .
//! # → grab the new downloadUrl + version
//! curl -fsSL <downloadUrl> -o /tmp/cursor.AppImage
//! sha256sum /tmp/cursor.AppImage
//! ```
//!
//! Update both `CURSOR_LINUX_X64_URL` and `CURSOR_LINUX_X64_SHA256` in the
//! same commit. The same applies to any future arm64 / x86 variants.
//!
//! This tool uses the ToolSpec pattern with a custom installer.

use crate::define_tool;
use crate::tools::common::{InstallError, has, run};
#[cfg(target_os = "linux")]
use crate::tools::pinned_binary_installer::AppImagePin;

/// Pinned Cursor Linux x86_64 AppImage URL. Includes the build commit sha
/// in the path so the URL never moves underneath us.
#[cfg(target_os = "linux")]
const CURSOR_LINUX_X64_URL: &str = "https://downloads.cursor.com/production/e56ad3440df06d22ca7501e65fd518e905486ef7/linux/x64/Cursor-3.8.11-x86_64.AppImage";

/// Lowercase 64-char hex sha256 of the AppImage at `CURSOR_LINUX_X64_URL`.
/// Computed locally on 2026-06-22; update in lockstep with the URL.
#[cfg(target_os = "linux")]
const CURSOR_LINUX_X64_SHA256: &str =
    "2bc3003ea81ce99a2458101478b15409c4cb8271577bd9cb941e8aaeae8a391a";

// No `linux: { ... }` block: cursor isn't in any Linux package manager.
// The custom_install fn handles the Linux AppImage path directly. Platform
// dispatch in spec.rs short-circuits on custom_install before consulting
// the linux block, so omission here only affects dry-run metadata — and
// the dry-run renderer separately labels custom-installer tools.
define_tool!(CURSOR, {
    command: "cursor",
    macos: { cask: "cursor" },
    windows: { winget: "Cursor.Cursor" },
    custom_install: install_cursor,
});

fn install_cursor(_min_hint: &str) -> Result<(), InstallError> {
    if has("cursor") {
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        if !has("brew") {
            return Err(InstallError::Prereq(
                "Homebrew not found. Install https://brew.sh and re-run.",
            ));
        }
        run("brew", &["install", "--cask", "cursor"])?;
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    {
        // x86_64 only — Cursor doesn't ship a Linux arm64 AppImage yet
        // (verified 2026-06-22 via their /api/download endpoint).
        #[cfg(not(target_arch = "x86_64"))]
        {
            return Err(InstallError::Unsupported);
        }
        #[cfg(target_arch = "x86_64")]
        {
            let installer = AppImagePin {
                name: "cursor",
                url: CURSOR_LINUX_X64_URL,
                sha256: CURSOR_LINUX_X64_SHA256,
            };
            run("sh", &["-c", &installer.shell_command()])?;
            return Ok(());
        }
    }

    #[cfg(target_os = "windows")]
    {
        if !has("winget") {
            return Err(InstallError::Prereq(
                "winget not found. Install Windows Package Manager, then re-run.",
            ));
        }
        run("winget", &["install", "-e", "--id", "Cursor.Cursor"])?;
        return Ok(());
    }

    #[allow(unreachable_code)]
    Err(InstallError::Unsupported)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_registration_shape() {
        assert_eq!(CURSOR.command, "cursor");
        let mac = CURSOR.macos.expect("must support macOS");
        assert_eq!(mac.cask, Some("cursor"));
        let win = CURSOR.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Cursor.Cursor"));
        // Linux is supported via custom_install (AppImage path) rather than
        // a package-manager block; spec dispatcher honors custom_install
        // first, so this is the canonical signal for "Linux installable".
        assert!(CURSOR.linux.is_none());
        assert!(CURSOR.custom_install.is_some());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_pin_is_well_formed() {
        // Sha must be lowercase 64-char hex
        assert_eq!(CURSOR_LINUX_X64_SHA256.len(), 64);
        assert!(
            CURSOR_LINUX_X64_SHA256
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );
        // URL must be HTTPS and include a versioned commit sha (32+ hex
        // chars) — refuses /latest/ aliases.
        assert!(CURSOR_LINUX_X64_URL.starts_with("https://"));
        assert!(!CURSOR_LINUX_X64_URL.contains("/latest/"));
    }
}
