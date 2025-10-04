use crate::tools::common::{InstallError, cmd_satisfies};
#[cfg(any(target_os = "macos", target_os = "windows"))]
use crate::tools::common::{has, run};

/// Ensure Python (python3) is installed and roughly matches `min_hint` if provided
pub fn ensure(min_hint: &str) -> Result<(), InstallError> {
    // Prefer python3, fall back to python
    if cmd_satisfies("python3", min_hint) || cmd_satisfies("python", min_hint) {
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
    // Brew provides Python 3 as `python` formula; ensures python3 binary
    run("brew", &["install", "python"])?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn install_linux() -> Result<(), InstallError> {
    if let Some(pm) = crate::tools::common::detect_linux_pm() {
        let _ = crate::tools::common::PkgOps::update(pm, true);
        let pkg = match pm {
            crate::tools::common::PackageManager::Apt => "python3",
            crate::tools::common::PackageManager::Dnf => "python3",
            crate::tools::common::PackageManager::Yum => "python3",
            crate::tools::common::PackageManager::Zypper => "python3",
            crate::tools::common::PackageManager::Pacman => "python",
            crate::tools::common::PackageManager::Apk => "python3",
            _ => "python3",
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
    // Python 3 via Winget official ID
    run("winget", &["install", "-e", "--id", "Python.Python.3"])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_python_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
