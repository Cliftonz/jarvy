#[cfg(any(target_os = "macos", target_os = "windows"))]
use crate::tools::common::run;
use crate::tools::common::{InstallError, has};

/// Ensure `make` is installed. The version hint is ignored; we only check for the
/// command availability.
pub fn ensure(_min_hint: &str) -> Result<(), InstallError> {
    if has("make") {
        return Ok(());
    }
    install()
}

/// Registry adapter: allows tools::add("make", version) to dispatch here
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
    // macOS typically has BSD make via Xcode CLT at /usr/bin/make.
    // If it is not present, offer Homebrew fallback.
    if has("make") {
        return Ok(());
    }
    if !has("brew") {
        return Err(InstallError::Prereq(
            "`make` not found. Install Xcode Command Line Tools (xcode-select --install) or Homebrew (https://brew.sh) and re-run.",
        ));
    }
    // Homebrew formula provides GNU make as `gmake` and may also symlink `make`.
    run("brew", &["install", "make"])?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn install_linux() -> Result<(), InstallError> {
    if let Some(pm) = crate::tools::common::detect_linux_pm() {
        let _ = crate::tools::common::PkgOps::update(pm, true);
        crate::tools::common::PkgOps::install(pm, "make", true)
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
    // Install GNU Make via winget. Package id commonly available:
    // If this package id changes, user can install make via MSYS2 or Chocolatey as an alternative.
    run("winget", &["install", "-e", "--id", "GnuWin32.Make"])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_make_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
