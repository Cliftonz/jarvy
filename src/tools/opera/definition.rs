//! opera - Opera browser (stable channel).
//!
//! macOS: Homebrew cask. Windows: winget. Linux: Opera's deb.opera.com apt
//! repository and rpm.opera.com dnf repository.

use crate::define_tool;
use crate::tools::common::{InstallError, has};

#[cfg(any(target_os = "macos", target_os = "windows"))]
use crate::tools::common::run;

#[cfg(target_os = "linux")]
use crate::tools::browser_repos::{AptRepo, DnfRepo, install_via_vendor_repo};

#[cfg(target_os = "linux")]
const APT: AptRepo = AptRepo {
    slug: "opera-stable",
    key_url: "https://deb.opera.com/archive.key",
    sources_line: "deb https://deb.opera.com/opera-stable/ stable non-free",
    package: "opera-stable",
};

#[cfg(target_os = "linux")]
const DNF: DnfRepo = DnfRepo {
    slug: "opera-stable",
    baseurl: "https://rpm.opera.com/rpm",
    gpgkey: "https://rpm.opera.com/rpmrepo.key",
    package: "opera-stable",
};

define_tool!(OPERA, {
    command: "opera",
    macos: { cask: "opera" },
    windows: { winget: "Opera.Opera" },
    custom_install: install_opera,
    category: "browser",
});

fn install_opera(
    _min_hint: &str,
    _ctx: &crate::tools::common::InstallContext,
) -> Result<(), InstallError> {
    if has("opera") {
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        if !has("brew") {
            return Err(InstallError::Prereq(
                "Homebrew not found. Install https://brew.sh and re-run.".into(),
            ));
        }
        run("brew", &["install", "--cask", "opera"])?;
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
        run("winget", &["install", "-e", "--id", "Opera.Opera"])?;
        return Ok(());
    }

    #[allow(unreachable_code)]
    Err(InstallError::Unsupported)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opera_registration_shape() {
        assert_eq!(OPERA.command, "opera");
        assert_eq!(OPERA.macos.unwrap().cask, Some("opera"));
        assert_eq!(OPERA.windows.unwrap().winget, Some("Opera.Opera"));
        assert!(OPERA.custom_install.is_some());
        assert_eq!(OPERA.category, Some("browser"));
    }
}
