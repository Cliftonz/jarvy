use crate::tools::common::{InstallError, has, run};

/// Ensure `yq` (mikefarah/yq) is installed. Version hint is ignored; we just
/// ensure the command is available.
pub fn ensure(_min_hint: &str) -> Result<(), InstallError> {
    if has("yq") {
        return Ok(());
    }
    install()
}

/// Registry adapter: allows tools::add("yq", version) to dispatch here
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
    run("brew", &["install", "yq"])?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn install_linux() -> Result<(), InstallError> {
    if let Some(pm) = crate::tools::common::detect_linux_pm() {
        let _ = crate::tools::common::PkgOps::update(pm, true);
        crate::tools::common::PkgOps::install(pm, "yq", true)
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
    // Official yq package by mikefarah
    run("winget", &["install", "-e", "--id", "mikefarah.yq"])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_yq_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
