use crate::outputs::error_message;
#[cfg(target_os = "linux")]
use crate::tools::common::detect_linux_pm;
use crate::tools::common::{InstallError, PackageManager, PkgOps, cmd_satisfies, run};

/// Registry adapter: allows tools::add("docker", version) to dispatch here
pub fn add_handler(min_hint: &str) -> Result<(), InstallError> {
    ensure(min_hint)
}

/// Ensure Docker is installed and at least roughly matches `min_hint`
/// (e.g., "24" → accepts 24.x)
fn ensure(min_hint: &str) -> Result<(), InstallError> {
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
    match run("brew", &["install", "--cask", "docker"]) {
        Ok(_) => Ok(()),
        Err(e) => {
            error_message("Docker");
            Err(e)
        }
    }
}

#[cfg(target_os = "windows")]
fn install_windows() -> Result<(), InstallError> {
    // Exact Winget ID for Docker Desktop
    PkgOps::install(PackageManager::Winget, "Docker.DockerDesktop")
}

#[cfg(target_os = "linux")]
fn install_linux() -> Result<(), InstallError> {
    match detect_linux_pm() {
        Some(PackageManager::Apt) => {
            run("sudo", &["apt-get", "update"])?;
            run(
                "sudo",
                &[
                    "apt-get",
                    "install",
                    "-y",
                    "docker-ce",
                    "docker-ce-cli",
                    "containerd.io",
                    "docker-buildx-plugin",
                    "docker-compose-plugin",
                ],
            )?;
            Ok(())
        }
        Some(PackageManager::Dnf | PackageManager::Yum) => {
            run(
                "sudo",
                &[
                    "dnf",
                    "remove",
                    "docker",
                    "docker-client",
                    "docker-client-latest",
                    "docker-common",
                    "docker-latest",
                    "docker-latest-logrotate",
                    "docker-logrotate",
                    "docker-selinux",
                    "docker-engine-selinux",
                    "docker-engine",
                ],
            )?;

            run("sudo", &["dnf", "install", "-y", "dnf-plugins-core"])?;
            run(
                "sudo",
                &[
                    "dnf-3",
                    "config-manager",
                    "--add-repo",
                    "https://download.docker.com/linux/fedora/docker-ce.repo",
                ],
            )?;

            Ok(())
        }
        Some(other) => {
            // Fallback to the distro package (less ideal, but keeps control in your hands)
            PkgOps::install(other, "docker").or(Err(InstallError::Prereq(
                "Consider adding the official Docker repo for your distro.",
            )))
        }
        None => Err(InstallError::Prereq(
            "No supported package manager on PATH.",
        )),
    }
}
