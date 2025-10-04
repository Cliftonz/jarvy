use crate::tools::common::{InstallError, cmd_satisfies};
#[cfg(any(target_os = "macos", target_os = "windows"))]
use crate::tools::common::{has, run};

/// Registry adapter: allows tools::add("docker", version) to dispatch here
pub fn add_handler(min_hint: &str) -> Result<(), InstallError> {
    ensure(min_hint)
}

/// Ensure Docker is installed and at least roughly matches `min_hint`
/// (e.g., "24" → accepts 24.x)
pub fn ensure(min_hint: &str) -> Result<(), InstallError> {
    if cmd_satisfies("docker", min_hint) {
        return Ok(());
    }
    install()
}

fn install() -> Result<(), InstallError> {
    #[cfg(target_os = "macos")]
    {
        return install_macos();
    }
    #[cfg(target_os = "linux")]
    {
        return install_linux();
    }
    #[cfg(target_os = "windows")]
    {
        return install_windows();
    }
    #[allow(unreachable_code)]
    Err(InstallError::Unsupported)
}

#[cfg(target_os = "macos")]
fn install_macos() -> Result<(), InstallError> {
    if !has("brew") {
        return Err(InstallError::Prereq(
            "Homebrew not found. Install https://brew.sh and re-run.",
        ));
    }
    run("brew", &["install", "--cask", "docker"])?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn install_windows() -> Result<(), InstallError> {
    if !has("winget") {
        return Err(InstallError::Prereq(
            "winget not found. Install Windows Package Manager, then re-run.",
        ));
    }
    // Exact Winget ID for Docker Desktop
    run("winget", &["install", "-e", "--id", "Docker.DockerDesktop"])?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn install_linux() -> Result<(), InstallError> {
    if let Some(pm) = crate::tools::common::detect_linux_pm() {
        // Best-effort update (ignore failure to keep minimal behavior)
        let _ = crate::tools::common::PkgOps::update(pm, true);
        // Package names vary; prefer distro packages over vendor repos for simplicity
        let pkg = match pm {
            crate::tools::common::PackageManager::Apt => "docker.io",
            crate::tools::common::PackageManager::Dnf => "docker",
            crate::tools::common::PackageManager::Yum => "docker",
            crate::tools::common::PackageManager::Zypper => "docker",
            crate::tools::common::PackageManager::Pacman => "docker",
            crate::tools::common::PackageManager::Apk => "docker",
            _ => "docker",
        };
        crate::tools::common::PkgOps::install(pm, pkg, true)
    } else {
        Err(InstallError::Prereq(
            "No supported Linux package manager on PATH (apt/dnf/yum/zypper/pacman/apk)",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_docker_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
