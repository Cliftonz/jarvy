use std::process::{Command, Output};


#[derive(thiserror::Error, Debug)]
pub enum InstallError {
    #[error("unsupported platform")]
    Unsupported,
    #[error("prerequisite missing: {0}")]
    Prereq(&'static str),
    #[error("command failed: {cmd} (code: {code:?})\n{stderr}")]
    CommandFailed { cmd: &'static str, code: Option<i32>, stderr: String },
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse error: {0}")]
    Parse(&'static str),
}

pub fn run(cmd: &'static str, args: &[&str]) -> Result<Output, InstallError> {
    let out = Command::new(cmd).args(args).output()?;
    if !out.status.success() {
        return Err(InstallError::CommandFailed {
            cmd,
            code: out.status.code(),
            stderr: String::from_utf8_lossy(&out.stderr).into(),
        });
    }
    Ok(out)
}

pub fn has(cmd: &str) -> bool {
    Command::new(cmd).arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

// Require one of multiple candidates (e.g., apt or apt-get)
pub fn require_any<'a>(candidates: &[&'a str], remediation: &'static str) -> Result<&'a str, InstallError> {
    for c in candidates {
        if has(c) { return Ok(*c); }
    }
    Err(InstallError::Prereq(remediation))
}

// crude semver probe like: "git version 2.44.0"
pub fn cmd_satisfies(cmd: &str, min_prefix: &str) -> bool {
    if let Ok(out) = Command::new(cmd).arg("--version").output() {
        let s = String::from_utf8_lossy(&out.stdout);
        return s.contains(min_prefix);
    }
    false
}


#[derive(Clone, Copy, Debug)]
pub enum PackageManager { Apt, Dnf, Yum, Zypper, Pacman, Apk, Brew, Winget }

#[cfg(target_os = "linux")]
pub fn detect_linux_pm() -> Option<PackageManager> {
    use std::{fs, process::Command};
    let has = |c| Command::new(c).arg("--version").output().map(|o| o.status.success()).unwrap_or(false);

    // (Optional) use /etc/os-release to bias choices when you need vendor repos
    // ID / ID_LIKE fields are the standard signals.  [oai_citation:0‡Freedesktop](https://www.freedesktop.org/software/systemd/man/os-release.html?utm_source=chatgpt.com) [oai_citation:1‡Debian Manpages](https://manpages.debian.org/trixie/systemd/os-release.5.en.html?utm_source=chatgpt.com)
    let _os_release = fs::read_to_string("/etc/os-release").unwrap_or_default();

    if has("apt-get") || has("apt") { return Some(PackageManager::Apt) }
    if has("dnf") { return Some(PackageManager::Dnf) }
    if has("yum") { return Some(PackageManager::Yum) }
    if has("zypper") { return Some(PackageManager::Zypper) }
    if has("pacman") { return Some(PackageManager::Pacman) }
    if has("apk") { return Some(PackageManager::Apk) }
    None
}

pub struct PkgOps;
impl PkgOps {
    pub fn update(pm: PackageManager) -> Result<(), InstallError> {
        match pm {
            PackageManager::Apt    => { run("sudo", &["apt-get", "update"])?; }
            PackageManager::Dnf    => { /* dnf auto-refreshes; optional */ }
            PackageManager::Yum    => { /* optional */ }
            PackageManager::Zypper => { run("sudo", &["zypper", "--non-interactive", "refresh"])?; }
            PackageManager::Pacman => { run("sudo", &["pacman", "-Sy"])?; }
            PackageManager::Apk    => { /* `apk add` refreshes on demand */ }
            _ => {}
        }
        Ok(())
    }
    pub fn install(pm: PackageManager, pkg: &str) -> Result<(), InstallError> {
        match pm {
            PackageManager::Apt    => run("sudo", &["apt-get", "install", "-y", pkg])?,
            PackageManager::Dnf    => run("sudo", &["dnf", "install", "-y", pkg])?,
            PackageManager::Yum    => run("sudo", &["yum", "install", "-y", pkg])?,
            PackageManager::Zypper => run("sudo", &["zypper", "--non-interactive", "install", "--no-confirm", pkg])?,
            PackageManager::Pacman => run("sudo", &["pacman", "--noconfirm", "-S", pkg])?,
            PackageManager::Apk    => run("sudo", &["apk", "add", pkg])?,
            PackageManager::Brew   => run("brew", &["install", pkg])?,
            PackageManager::Winget => run("winget", &["install", "-e", "--id", pkg])?,
        }; Ok(())
    }
}