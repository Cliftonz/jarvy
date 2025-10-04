use crate::tools::common::{InstallError, cmd_satisfies};
#[cfg(any(target_os = "macos", target_os = "windows"))]
use crate::tools::common::{has, run};

/// Ensure Go is installed and roughly matches `min_hint` if provided
pub fn ensure(min_hint: &str) -> Result<(), InstallError> {
    if cmd_satisfies("go", min_hint) {
        return Ok(());
    }
    install()
}

/// Registry adapter
pub fn add_handler(min_hint: &str) -> Result<(), InstallError> {
    ensure(min_hint)
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
    run("brew", &["install", "go"])?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn install_linux() -> Result<(), InstallError> {
    if let Some(pm) = crate::tools::common::detect_linux_pm() {
        let _ = crate::tools::common::PkgOps::update(pm, true);
        let pkg = match pm {
            crate::tools::common::PackageManager::Apt => "golang",
            crate::tools::common::PackageManager::Dnf => "golang",
            crate::tools::common::PackageManager::Yum => "golang",
            crate::tools::common::PackageManager::Zypper => "go",
            crate::tools::common::PackageManager::Pacman => "go",
            crate::tools::common::PackageManager::Apk => "go",
            _ => "golang",
        };
        crate::tools::common::PkgOps::install(pm, pkg, true)
    } else {
        Err(InstallError::Prereq(
            "No supported Linux package manager on PATH (apt/dnf/yum/zypper/pacman/apk)",
        ))
    }
}

#[cfg(target_os = "windows")]
fn install_windows() -> Result<(), InstallError> {
    if !has("winget") {
        return Err(InstallError::Prereq(
            "winget not found. Install Windows Package Manager, then re-run.",
        ));
    }
    // Go via Winget
    run("winget", &["install", "-e", "--id", "GoLang.Go"])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_go_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
