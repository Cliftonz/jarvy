//! chrome - Google Chrome (stable channel).
//!
//! macOS: Homebrew cask. Windows: winget. Linux: Google's vendor apt/dnf
//! repository (`google-chrome-stable`). Linux arm64 is unsupported upstream —
//! Google only ships x86_64 debs/rpms, so arm64 Linux boxes surface as
//! `Unsupported` via [`vendor_repos::install_via_vendor_repo`].

use crate::define_tool;
use crate::tools::common::{InstallError, has};

#[cfg(any(target_os = "macos", target_os = "windows"))]
use crate::tools::common::run;

#[cfg(target_os = "linux")]
use crate::tools::vendor_repos::{AptRepo, DnfRepo, install_via_vendor_repo};

#[cfg(target_os = "linux")]
const APT: AptRepo = AptRepo {
    slug: "google-chrome",
    key_url: "https://dl.google.com/linux/linux_signing_key.pub",
    raw_key_url: None,
    sources_line: "deb https://dl.google.com/linux/chrome/deb/ stable main",
    package: "google-chrome-stable",
};

#[cfg(target_os = "linux")]
const DNF: DnfRepo = DnfRepo {
    slug: "google-chrome",
    baseurl: "https://dl.google.com/linux/chrome/rpm/stable/x86_64",
    gpgkey: "https://dl.google.com/linux/linux_signing_key.pub",
    gpgkey2: None,
    package: "google-chrome-stable",
};

define_tool!(CHROME, {
    command: "google-chrome",
    macos: { cask: "google-chrome" },
    windows: { winget: "Google.Chrome" },
    custom_install: install_chrome,
    category: "browser",
});

fn install_chrome(
    _min_hint: &str,
    _ctx: &crate::tools::common::InstallContext,
) -> Result<(), InstallError> {
    if has("google-chrome") {
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        if !has("brew") {
            return Err(InstallError::Prereq(
                "Homebrew not found. Install https://brew.sh and re-run.".into(),
            ));
        }
        run("brew", &["install", "--cask", "google-chrome"])?;
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
        run("winget", &["install", "-e", "--id", "Google.Chrome"])?;
        return Ok(());
    }

    #[allow(unreachable_code)]
    Err(InstallError::Unsupported)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chrome_registration_shape() {
        assert_eq!(CHROME.command, "google-chrome");
        assert_eq!(CHROME.macos.unwrap().cask, Some("google-chrome"));
        assert_eq!(CHROME.windows.unwrap().winget, Some("Google.Chrome"));
        assert!(CHROME.custom_install.is_some());
        assert_eq!(CHROME.category, Some("browser"));
    }
}
