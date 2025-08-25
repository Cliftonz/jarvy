use std::process::{Command, Output};

#[derive(thiserror::Error, Debug)]
pub enum InstallError {
    #[error("unsupported platform")]
    Unsupported,
    #[error("prerequisite missing: {0}")]
    Prereq(&'static str),
    #[error("command failed: {cmd} (code: {code:?})\n{stderr}")]
    CommandFailed {
        cmd: String,
        code: Option<i32>,
        stderr: String,
    },
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse error: {0}")]
    Parse(&'static str),
}

// OS enum for config keys and runtime resolution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Os {
    Linux,
    Macos,
    Windows,
}

// Determine the current OS as our enum
pub fn current_os() -> Os {
    #[cfg(target_os = "linux")]
    {
        Os::Linux
    }
    #[cfg(target_os = "macos")]
    {
        Os::Macos
    }
    #[cfg(target_os = "windows")]
    {
        Os::Windows
    }
}

pub fn run(cmd: &str, args: &[&str]) -> Result<Output, InstallError> {
    let out = Command::new(cmd).args(args).output().map_err(|e| {
        use std::io::ErrorKind::*;
        match e.kind() {
            NotFound => InstallError::Prereq("required command not found on PATH"),
            PermissionDenied => InstallError::Prereq("operation requires elevated privileges"),
            _ => InstallError::Io(e),
        }
    })?;

    if !out.status.success() {
        // Try to capture stderr for easier diagnostics.
        return Err(InstallError::CommandFailed {
            cmd: cmd.to_string(),
            code: out.status.code(),
            stderr: String::from_utf8_lossy(&out.stderr).into(),
        });
    }
    Ok(out)
}

// Run a command, prefixing with sudo if configured and applicable (non-Windows)
pub fn run_maybe_sudo(use_sudo: bool, cmd: &str, args: &[&str]) -> Result<Output, InstallError> {
    match current_os() {
        Os::Windows => run(cmd, args),
        Os::Linux | Os::Macos => {
            if use_sudo {
                // sudo <cmd> <args...>
                let mut all = Vec::with_capacity(1 + args.len());
                all.push(cmd);
                all.extend_from_slice(args);
                run("sudo", &all)
            } else {
                run(cmd, args)
            }
        }
    }
}

pub fn has(cmd: &str) -> bool {
    Command::new(cmd)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// Require a single command to exist on PATH, otherwise return a Prereq error with remediation.
pub fn require(cmd: &str, remediation: &'static str) -> Result<(), InstallError> {
    if has(cmd) {
        Ok(())
    } else {
        Err(InstallError::Prereq(remediation))
    }
}

// Require one of multiple candidates (e.g., apt or apt-get)
pub fn require_any<'a>(
    candidates: &[&'a str],
    remediation: &'static str,
) -> Result<&'a str, InstallError> {
    for c in candidates {
        if has(c) {
            return Ok(*c);
        }
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
pub enum PackageManager {
    Apt,
    Dnf,
    Yum,
    Zypper,
    Pacman,
    Apk,
    Brew,
    Winget,
}

#[cfg(target_os = "linux")]
pub fn detect_linux_pm() -> Option<PackageManager> {
    use std::{fs, process::Command};
    let has = |c| {
        Command::new(c)
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    };

    // (Optional) use /etc/os-release to bias choices when you need vendor repos
    // ID / ID_LIKE fields are the standard signals.  [oai_citation:0‡Freedesktop](https://www.freedesktop.org/software/systemd/man/os-release.html?utm_source=chatgpt.com) [oai_citation:1‡Debian Manpages](https://manpages.debian.org/trixie/systemd/os-release.5.en.html?utm_source=chatgpt.com)
    let _os_release = fs::read_to_string("/etc/os-release").unwrap_or_default();

    if has("apt-get") || has("apt") {
        return Some(PackageManager::Apt);
    }
    if has("dnf") {
        return Some(PackageManager::Dnf);
    }
    if has("yum") {
        return Some(PackageManager::Yum);
    }
    if has("zypper") {
        return Some(PackageManager::Zypper);
    }
    if has("pacman") {
        return Some(PackageManager::Pacman);
    }
    if has("apk") {
        return Some(PackageManager::Apk);
    }
    None
}

pub struct PkgOps {
    name: &'static str,
}

impl PkgOps {
    // use_sudo should come from config.use_sudo()
    pub fn update(pm: PackageManager, use_sudo: bool) -> Result<(), InstallError> {
        match pm {
            PackageManager::Apt => {
                // Ensure prerequisites exist before attempting the update
                let apt = require_any(&["apt-get", "apt"], "apt is required to update packages")?;
                if use_sudo {
                    require("sudo", "sudo is required to update packages")?;
                }
                run_maybe_sudo(use_sudo, apt, &["update"])?;
            }
            PackageManager::Dnf => { /* dnf auto-refreshes; optional */ }
            PackageManager::Yum => { /* optional */ }
            PackageManager::Zypper => {
                require("zypper", "zypper is required to update packages")?;
                if use_sudo {
                    require("sudo", "sudo is required to update packages")?;
                }
                run_maybe_sudo(use_sudo, "zypper", &["--non-interactive", "refresh"])?;
            }
            PackageManager::Pacman => {
                require("pacman", "pacman is required to update packages")?;
                if use_sudo {
                    require("sudo", "sudo is required to update packages")?;
                }
                run_maybe_sudo(use_sudo, "pacman", &["-Sy"])?;
            }
            PackageManager::Apk => { /* `apk add` refreshes on demand */ }
            _ => {}
        }
        Ok(())
    }

    // use_sudo should come from config.use_sudo()
    pub fn install(pm: PackageManager, pkg: &str, use_sudo: bool) -> Result<(), InstallError> {
        match pm {
            PackageManager::Apt => {
                let apt = require_any(&["apt-get", "apt"], "apt is required to install packages")?;
                if use_sudo {
                    require("sudo", "sudo is required to install packages")?;
                }
                run_maybe_sudo(use_sudo, apt, &["install", "-y", pkg])?
            }
            PackageManager::Dnf => {
                require("dnf", "dnf is required to install packages")?;
                if use_sudo {
                    require("sudo", "sudo is required to install packages")?;
                }
                run_maybe_sudo(use_sudo, "dnf", &["install", "-y", pkg])?
            }
            PackageManager::Yum => {
                require("yum", "yum is required to install packages")?;
                if use_sudo {
                    require("sudo", "sudo is required to install packages")?;
                }
                run_maybe_sudo(use_sudo, "yum", &["install", "-y", pkg])?
            }
            PackageManager::Zypper => {
                require("zypper", "zypper is required to install packages")?;
                if use_sudo {
                    require("sudo", "sudo is required to install packages")?;
                }
                run_maybe_sudo(
                    use_sudo,
                    "zypper",
                    &["--non-interactive", "install", "--no-confirm", pkg],
                )?
            }
            PackageManager::Pacman => {
                require("pacman", "pacman is required to install packages")?;
                if use_sudo {
                    require("sudo", "sudo is required to install packages")?;
                }
                run_maybe_sudo(use_sudo, "pacman", &["--noconfirm", "-S", pkg])?
            }
            PackageManager::Apk => {
                require("apk", "apk is required to install packages")?;
                if use_sudo {
                    require("sudo", "sudo is required to install packages")?;
                }
                run_maybe_sudo(use_sudo, "apk", &["add", pkg])?
            }
            // These package managers generally do not require sudo by design here
            PackageManager::Brew => {
                require("brew", "Homebrew is required to install packages")?;
                run("brew", &["install", pkg])?
            }
            PackageManager::Winget => {
                require("winget", "Winget is required to install packages")?;
                run("winget", &["install", "-e", "--id", pkg])?
            }
        };
        Ok(())
    }
}
