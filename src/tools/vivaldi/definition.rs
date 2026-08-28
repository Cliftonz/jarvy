//! vivaldi - Vivaldi browser (stable channel).
//!
//! macOS: Homebrew cask. Windows: winget. Linux: Vivaldi's
//! repo.vivaldi.com apt/dnf repository. arm64 debs are published so
//! multiarch Debian/Ubuntu boxes are covered.

use crate::define_tool;
use crate::tools::common::{InstallError, has};

#[cfg(any(target_os = "macos", target_os = "windows"))]
use crate::tools::common::run;

#[cfg(target_os = "linux")]
use crate::tools::vendor_repos::{AptRepo, DnfRepo, install_via_vendor_repo};

#[cfg(target_os = "linux")]
const APT: AptRepo = AptRepo {
    slug: "vivaldi",
    key_url: "https://repo.vivaldi.com/archive/linux_signing_key.pub",
    raw_key_url: None,
    sources_line: "deb https://repo.vivaldi.com/archive/deb/ stable main",
    package: "vivaldi-stable",
};

#[cfg(target_os = "linux")]
const DNF: DnfRepo = DnfRepo {
    slug: "vivaldi",
    baseurl: "https://repo.vivaldi.com/archive/rpm/x86_64",
    gpgkey: "https://repo.vivaldi.com/archive/linux_signing_key.pub",
    gpgkey2: None,
    package: "vivaldi-stable",
};

define_tool!(VIVALDI, {
    command: "vivaldi",
    macos: { cask: "vivaldi" },
    windows: { winget: "Vivaldi.Vivaldi" },
    custom_install: install_vivaldi,
    category: "browser",
});

fn install_vivaldi(
    _min_hint: &str,
    _ctx: &crate::tools::common::InstallContext,
) -> Result<(), InstallError> {
    if has("vivaldi") || has("vivaldi-stable") {
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        if !has("brew") {
            return Err(InstallError::Prereq(
                "Homebrew not found. Install https://brew.sh and re-run.".into(),
            ));
        }
        run("brew", &["install", "--cask", "vivaldi"])?;
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
        run("winget", &["install", "-e", "--id", "Vivaldi.Vivaldi"])?;
        return Ok(());
    }

    #[allow(unreachable_code)]
    Err(InstallError::Unsupported)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vivaldi_registration_shape() {
        assert_eq!(VIVALDI.command, "vivaldi");
        assert_eq!(VIVALDI.macos.unwrap().cask, Some("vivaldi"));
        assert_eq!(VIVALDI.windows.unwrap().winget, Some("Vivaldi.Vivaldi"));
        assert!(VIVALDI.custom_install.is_some());
        assert_eq!(VIVALDI.category, Some("browser"));
    }
}
