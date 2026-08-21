//! edge - Microsoft Edge (stable channel).
//!
//! macOS: Homebrew cask. Windows: winget. Linux: Microsoft's packages.microsoft.com
//! apt/dnf repository (`microsoft-edge-stable`). x86_64 only upstream.

use crate::define_tool;
use crate::tools::common::{InstallError, has};

#[cfg(any(target_os = "macos", target_os = "windows"))]
use crate::tools::common::run;

#[cfg(target_os = "linux")]
use crate::tools::browser_repos::{AptRepo, DnfRepo, install_via_vendor_repo};

#[cfg(target_os = "linux")]
const APT: AptRepo = AptRepo {
    slug: "microsoft-edge",
    key_url: "https://packages.microsoft.com/keys/microsoft.asc",
    sources_line: "deb https://packages.microsoft.com/repos/edge stable main",
    package: "microsoft-edge-stable",
};

#[cfg(target_os = "linux")]
const DNF: DnfRepo = DnfRepo {
    slug: "microsoft-edge",
    baseurl: "https://packages.microsoft.com/yumrepos/edge",
    gpgkey: "https://packages.microsoft.com/keys/microsoft.asc",
    package: "microsoft-edge-stable",
};

define_tool!(EDGE, {
    command: "microsoft-edge",
    macos: { cask: "microsoft-edge" },
    windows: { winget: "Microsoft.Edge" },
    custom_install: install_edge,
    category: "browser",
});

fn install_edge(
    _min_hint: &str,
    _ctx: &crate::tools::common::InstallContext,
) -> Result<(), InstallError> {
    if has("microsoft-edge") || has("microsoft-edge-stable") {
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        if !has("brew") {
            return Err(InstallError::Prereq(
                "Homebrew not found. Install https://brew.sh and re-run.".into(),
            ));
        }
        run("brew", &["install", "--cask", "microsoft-edge"])?;
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
        run("winget", &["install", "-e", "--id", "Microsoft.Edge"])?;
        return Ok(());
    }

    #[allow(unreachable_code)]
    Err(InstallError::Unsupported)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edge_registration_shape() {
        assert_eq!(EDGE.command, "microsoft-edge");
        assert_eq!(EDGE.macos.unwrap().cask, Some("microsoft-edge"));
        assert_eq!(EDGE.windows.unwrap().winget, Some("Microsoft.Edge"));
        assert!(EDGE.custom_install.is_some());
        assert_eq!(EDGE.category, Some("browser"));
    }
}
