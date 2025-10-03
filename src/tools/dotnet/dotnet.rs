#[cfg(any(target_os = "macos", target_os = "windows"))]
use crate::tools::common::run;
use crate::tools::common::{InstallError, has};

/// Ensure `dotnet` (the .NET SDK/CLI) is available. No strict version match.
pub fn ensure(_min_hint: &str) -> Result<(), InstallError> {
    if has("dotnet") {
        return Ok(());
    }
    install()
}

/// Registry adapter: allows tools::add("dotnet", version) to dispatch here
pub fn add_handler(min_hint: &str) -> Result<(), InstallError> {
    let _ = min_hint;
    ensure("")
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
    // Use cask for .NET SDK on macOS
    run("brew", &["install", "--cask", "dotnet-sdk"])?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn install_linux() -> Result<(), InstallError> {
    if let Some(pm) = crate::tools::common::detect_linux_pm() {
        let _ = crate::tools::common::PkgOps::update(pm, true);
        // Package availability varies by distro; try a reasonable default name
        let pkg = match pm {
            crate::tools::common::PackageManager::Apt => "dotnet-sdk-8.0",
            crate::tools::common::PackageManager::Dnf => "dotnet-sdk",
            crate::tools::common::PackageManager::Yum => "dotnet-sdk",
            crate::tools::common::PackageManager::Zypper => "dotnet-sdk",
            crate::tools::common::PackageManager::Pacman => "dotnet-sdk",
            crate::tools::common::PackageManager::Apk => "dotnet-sdk",
            _ => "dotnet-sdk",
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
    // Install .NET SDK 8 (LTS at time of writing). Adjust as needed.
    run(
        "winget",
        &["install", "-e", "--id", "Microsoft.DotNet.SDK.8"],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_dotnet_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
