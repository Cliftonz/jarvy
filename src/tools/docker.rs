use crate::tools::common::{InstallError, PkgOps, PackageManager, run, detect_linux_pm};

#[cfg(target_os = "macos")]
fn install_macos() -> Result<(), InstallError> {
    // Desktop via Homebrew cask
    run("brew", &["install", "--cask", "docker"])?;
    Ok(())
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
            // Vendor-repo flow (Ubuntu/Debian): add Docker’s APT repo, then install.
            // (This is where you keep the precise steps.)
            run("sudo", &["apt-get", "update"])?; /* ...add repo & key..., then: */
            run("sudo", &["apt-get", "install", "-y",
                "docker-ce","docker-ce-cli","containerd.io",
                "docker-buildx-plugin","docker-compose-plugin"
            ])?;
            Ok(())
        }
        Some(PackageManager::Dnf | PackageManager::Yum) => {
            // Fedora/RHEL flow (Docker RPM repo)...
            // (Add repo, then `dnf install -y docker-ce ...`)

            run("sudo", &["dnf", "install", "-y", "docker-ce"])?;

            Ok(())
        }
        Some(other) => {
            // Fallback to the distro package (less ideal, but keeps control in your hands)
            PkgOps::install(other, "docker").or(Err(InstallError::Prereq(
                "Consider adding the official Docker repo for your distro."
            )))
        }
        None => Err(InstallError::Prereq("No supported package manager on PATH.")),
    }
}