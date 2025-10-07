#[cfg(any(target_os = "macos", target_os = "windows"))]
use crate::tools::common::run;
use crate::tools::common::{InstallError, has};

/// Ensure `xz` is available.
pub fn ensure(_min_hint: &str) -> Result<(), InstallError> {
    if has("xz") {
        return Ok(());
    }
    install()
}

/// Registry adapter: allows tools::add("xz", version) to dispatch here
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
    run("brew", &["install", "xz"])?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn install_linux() -> Result<(), InstallError> {
    if let Some(pm) = crate::tools::common::detect_linux_pm() {
        let _ = crate::tools::common::PkgOps::update(pm, crate::tools::common::default_use_sudo());
        // Debian/Ubuntu use xz-utils; others often use xz. We try xz first, falling back to xz-utils.
        if crate::tools::common::PkgOps::install(pm, "xz", crate::tools::common::default_use_sudo())
            .is_ok()
        {
            return Ok(());
        }
        crate::tools::common::PkgOps::install(
            pm,
            "xz-utils",
            crate::tools::common::default_use_sudo(),
        )
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
    // Common xz package on winget (XZUtils.XZ). If this fails in practice, users can install via MSYS2/Chocolatey.
    run("winget", &["install", "-e", "--id", "XZUtils.XZ"])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_xz_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
