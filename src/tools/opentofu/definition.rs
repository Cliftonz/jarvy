//! opentofu - open-source Terraform alternative
//!
//! macOS: Homebrew. Windows: winget. Linux: OpenTofu isn't in Debian/
//! Ubuntu's or Fedora/RHEL's default repos, so apt/dnf go through
//! OpenTofu's vendor repository (see `vendor_repos`). Arch's official
//! `extra` repo already carries `opentofu`, so pacman installs directly.
//! Alpine's `opentofu` package is testing/edge-only (not stable) — no
//! apk route is declared, so Alpine surfaces a clean unsupported error
//! rather than a silently-broken install.
//!
//! Note: the command is `tofu`, not `opentofu`; the apt/dnf package
//! name is also `tofu`, not `opentofu` (pacman's package IS named
//! `opentofu` — vendor-specific naming, not a typo).

use crate::define_tool;
use crate::tools::common::{InstallContext, InstallError};

#[cfg(target_os = "linux")]
use crate::tools::common::{has, run};

#[cfg(target_os = "linux")]
use crate::tools::vendor_repos::{AptRepo, DnfRepo, install_via_vendor_repo};

#[cfg(target_os = "linux")]
const APT: AptRepo = AptRepo {
    slug: "opentofu",
    key_url: "https://packages.opentofu.org/opentofu/tofu/gpgkey",
    raw_key_url: Some("https://get.opentofu.org/opentofu.gpg"),
    sources_line: "deb https://packages.opentofu.org/opentofu/tofu/any/ any main",
    package: "tofu",
};

#[cfg(target_os = "linux")]
const DNF: DnfRepo = DnfRepo {
    slug: "opentofu",
    baseurl: "https://packages.opentofu.org/opentofu/tofu/rpm_any/rpm_any/$basearch",
    gpgkey: "https://get.opentofu.org/opentofu.gpg",
    gpgkey2: Some("https://packages.opentofu.org/opentofu/tofu/gpgkey"),
    package: "tofu",
};

define_tool!(OPENTOFU, {
    command: "tofu",
    macos: { brew: "opentofu" },
    windows: { winget: "OpenTofu.OpenTofu" },
    bsd: { pkg: "opentofu" },
    custom_install: install_opentofu,
});

fn install_opentofu(_min_hint: &str, _ctx: &InstallContext) -> Result<(), InstallError> {
    #[cfg(target_os = "linux")]
    {
        if has("apt-get") || has("dnf") {
            return install_via_vendor_repo(Some(&APT), Some(&DNF));
        }
        if has("pacman") {
            run("pacman", &["-S", "--noconfirm", "opentofu"])?;
            return Ok(());
        }
    }
    OPENTOFU.install_platform()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opentofu_registration_shape() {
        assert_eq!(OPENTOFU.command, "tofu");
        let mac = OPENTOFU.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("opentofu"));
        let win = OPENTOFU.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("OpenTofu.OpenTofu"));
        assert!(OPENTOFU.custom_install.is_some());
        assert!(
            OPENTOFU.linux.is_none(),
            "Linux is handled entirely through custom_install (vendor repo for apt/dnf, direct pacman, no apk)"
        );
    }
}
