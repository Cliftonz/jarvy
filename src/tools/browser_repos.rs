#![allow(dead_code)] // scripts + repo descriptors exercised on Linux only; tests cover the pure planners

//! Shared Linux vendor-repo installer for browsers that aren't in default distro
//! package sources (Chrome, Brave, Edge, Opera, Vivaldi). Adds the vendor's apt
//! or dnf repository, then installs via the platform package manager.
//!
//! The pure planner [`vendor_apt_script`] / [`vendor_dnf_script`] emits the shell
//! command idempotently so re-runs are safe. Executors are Linux-only.
//!
//! Vendor repo bootstrap requires `sudo` + `curl` + `apt-get` (Debian family) or
//! `dnf` (Red Hat family). Missing prerequisites surface as
//! `InstallError::Prereq`.

use crate::tools::common::InstallError;

/// Debian/Ubuntu vendor-apt repository descriptor.
#[derive(Debug, Clone, Copy)]
pub struct AptRepo {
    /// Short slug used for keyring filename and sources.list.d entry
    /// (`google-chrome`, `brave-browser`, `microsoft-edge`, …).
    pub slug: &'static str,
    /// HTTPS URL of the vendor's GPG signing key (armored `.pub` / `.gpg`).
    pub key_url: &'static str,
    /// Full sources.list line body (starting with `deb ` — the
    /// `[signed-by=…]` fragment is injected by the planner).
    pub sources_line: &'static str,
    /// apt package name to install.
    pub package: &'static str,
}

/// Red Hat family vendor-dnf repository descriptor.
#[derive(Debug, Clone, Copy)]
pub struct DnfRepo {
    pub slug: &'static str,
    /// HTTPS baseurl for the RPM repo.
    pub baseurl: &'static str,
    /// HTTPS URL of the vendor's GPG signing key.
    pub gpgkey: &'static str,
    /// dnf package name to install.
    pub package: &'static str,
}

/// Emit an idempotent shell script that bootstraps a vendor apt repo and
/// installs `repo.package`. Pure — testable without touching disk.
pub fn vendor_apt_script(repo: &AptRepo) -> String {
    let keyring = format!("/usr/share/keyrings/{}-archive-keyring.gpg", repo.slug);
    let list_file = format!("/etc/apt/sources.list.d/{}.list", repo.slug);
    format!(
        "set -eu\n\
         if ! command -v curl >/dev/null 2>&1; then echo 'curl is required' >&2; exit 1; fi\n\
         if ! command -v apt-get >/dev/null 2>&1; then echo 'apt-get is required' >&2; exit 1; fi\n\
         sudo install -d -m 0755 /usr/share/keyrings\n\
         if [ ! -s '{keyring}' ]; then\n\
             curl -fsSL '{key_url}' | sudo gpg --dearmor --yes -o '{keyring}'\n\
             sudo chmod 0644 '{keyring}'\n\
         fi\n\
         echo '{sources} [signed-by={keyring}]' > /tmp/{slug}.list.tmp\n\
         sudo install -m 0644 /tmp/{slug}.list.tmp '{list_file}'\n\
         rm -f /tmp/{slug}.list.tmp\n\
         sudo apt-get update\n\
         sudo DEBIAN_FRONTEND=noninteractive apt-get install -y {package}\n",
        keyring = keyring,
        list_file = list_file,
        key_url = repo.key_url,
        sources = repo.sources_line,
        slug = repo.slug,
        package = repo.package,
    )
}

/// Emit an idempotent shell script that writes a vendor dnf repo file and
/// installs `repo.package`. Pure — testable without touching disk.
pub fn vendor_dnf_script(repo: &DnfRepo) -> String {
    let repo_file = format!("/etc/yum.repos.d/{}.repo", repo.slug);
    let body = format!(
        "[{slug}]\n\
         name={slug}\n\
         baseurl={baseurl}\n\
         enabled=1\n\
         gpgcheck=1\n\
         gpgkey={gpgkey}\n",
        slug = repo.slug,
        baseurl = repo.baseurl,
        gpgkey = repo.gpgkey,
    );
    format!(
        "set -eu\n\
         if ! command -v dnf >/dev/null 2>&1; then echo 'dnf is required' >&2; exit 1; fi\n\
         cat <<'EOF' > /tmp/{slug}.repo.tmp\n{body}EOF\n\
         sudo install -m 0644 /tmp/{slug}.repo.tmp '{repo_file}'\n\
         rm -f /tmp/{slug}.repo.tmp\n\
         sudo dnf install -y {package}\n",
        slug = repo.slug,
        body = body,
        repo_file = repo_file,
        package = repo.package,
    )
}

/// Install a browser on Linux via its vendor's apt or dnf repository.
///
/// Detects the platform package manager, picks the matching repo descriptor,
/// and shells out. Both repo descriptors are required so a distro without
/// either apt nor dnf (arch, alpine) surfaces a clean `Unsupported` error.
#[cfg(target_os = "linux")]
pub fn install_via_vendor_repo(
    apt: Option<&AptRepo>,
    dnf: Option<&DnfRepo>,
) -> Result<(), InstallError> {
    use crate::tools::common::{has, run};

    if has("apt-get") {
        let repo = apt.ok_or(InstallError::Unsupported)?;
        return run("sh", &["-c", &vendor_apt_script(repo)]).map(|_| ());
    }
    if has("dnf") {
        let repo = dnf.ok_or(InstallError::Unsupported)?;
        return run("sh", &["-c", &vendor_dnf_script(repo)]).map(|_| ());
    }
    Err(InstallError::Prereq(
        "vendor-repo install needs apt-get or dnf on PATH".into(),
    ))
}

#[cfg(not(target_os = "linux"))]
#[allow(dead_code)]
pub fn install_via_vendor_repo(
    _apt: Option<&AptRepo>,
    _dnf: Option<&DnfRepo>,
) -> Result<(), InstallError> {
    Err(InstallError::Unsupported)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHROME_APT: AptRepo = AptRepo {
        slug: "google-chrome",
        key_url: "https://dl.google.com/linux/linux_signing_key.pub",
        sources_line: "deb https://dl.google.com/linux/chrome/deb/ stable main",
        package: "google-chrome-stable",
    };

    const CHROME_DNF: DnfRepo = DnfRepo {
        slug: "google-chrome",
        baseurl: "https://dl.google.com/linux/chrome/rpm/stable/x86_64",
        gpgkey: "https://dl.google.com/linux/linux_signing_key.pub",
        package: "google-chrome-stable",
    };

    #[test]
    fn apt_script_is_idempotent_and_pins_keyring() {
        let s = vendor_apt_script(&CHROME_APT);
        assert!(s.contains("set -eu"));
        assert!(s.contains("[signed-by=/usr/share/keyrings/google-chrome-archive-keyring.gpg]"));
        assert!(s.contains("if [ ! -s '/usr/share/keyrings/google-chrome-archive-keyring.gpg' ]"));
        assert!(s.contains("apt-get install -y google-chrome-stable"));
        assert!(s.contains("DEBIAN_FRONTEND=noninteractive"));
    }

    #[test]
    fn dnf_script_writes_repo_file_and_installs() {
        let s = vendor_dnf_script(&CHROME_DNF);
        assert!(s.contains("[google-chrome]"));
        assert!(s.contains("gpgcheck=1"));
        assert!(s.contains("baseurl=https://dl.google.com/linux/chrome/rpm/stable/x86_64"));
        assert!(s.contains("/etc/yum.repos.d/google-chrome.repo"));
        assert!(s.contains("dnf install -y google-chrome-stable"));
    }

    #[test]
    fn apt_and_dnf_scripts_only_use_https_urls() {
        for s in [
            vendor_apt_script(&CHROME_APT),
            vendor_dnf_script(&CHROME_DNF),
        ] {
            assert!(!s.contains("http://"), "http url leaked: {s}");
        }
    }
}
