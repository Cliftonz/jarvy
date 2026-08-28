//! brave - Brave browser (stable channel).
//!
//! macOS: Homebrew cask. Windows: winget. Linux: Brave's vendor apt/dnf
//! repository. arm64 debs are also published on the apt repo, so multiarch
//! Debian/Ubuntu boxes work.

use crate::define_tool;
use crate::tools::common::{InstallError, has};

#[cfg(any(target_os = "macos", target_os = "windows"))]
use crate::tools::common::run;

#[cfg(target_os = "linux")]
use crate::tools::vendor_repos::{AptRepo, DnfRepo, install_via_vendor_repo};

#[cfg(target_os = "linux")]
const APT: AptRepo = AptRepo {
    slug: "brave-browser",
    key_url: "https://brave-browser-apt-release.s3.brave.com/brave-browser-archive-keyring.gpg",
    raw_key_url: None,
    sources_line: "deb https://brave-browser-apt-release.s3.brave.com/ stable main",
    package: "brave-browser",
};

#[cfg(target_os = "linux")]
const DNF: DnfRepo = DnfRepo {
    slug: "brave-browser",
    baseurl: "https://brave-browser-rpm-release.s3.brave.com/x86_64/",
    gpgkey: "https://brave-browser-rpm-release.s3.brave.com/brave-core.asc",
    gpgkey2: None,
    package: "brave-browser",
};

define_tool!(BRAVE, {
    command: "brave-browser",
    macos: { cask: "brave-browser" },
    windows: { winget: "Brave.Brave" },
    custom_install: install_brave,
    category: "browser",
});

fn install_brave(
    _min_hint: &str,
    _ctx: &crate::tools::common::InstallContext,
) -> Result<(), InstallError> {
    if has("brave-browser") {
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        if !has("brew") {
            return Err(InstallError::Prereq(
                "Homebrew not found. Install https://brew.sh and re-run.".into(),
            ));
        }
        run("brew", &["install", "--cask", "brave-browser"])?;
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    {
        return install_via_vendor_repo(Some(&APT), Some(&DNF));
    }

    #[cfg(target_os = "windows")]
    {
        if !has("winget") {
            return Err(InstallError::Prereq(
                "winget not found. Install Windows Package Manager, then re-run.".into(),
            ));
        }
        run("winget", &["install", "-e", "--id", "Brave.Brave"])?;
        return Ok(());
    }

    #[allow(unreachable_code)]
    Err(InstallError::Unsupported)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brave_registration_shape() {
        assert_eq!(BRAVE.command, "brave-browser");
        assert_eq!(BRAVE.macos.unwrap().cask, Some("brave-browser"));
        assert_eq!(BRAVE.windows.unwrap().winget, Some("Brave.Brave"));
        assert!(BRAVE.custom_install.is_some());
        assert_eq!(BRAVE.category, Some("browser"));
    }
}
