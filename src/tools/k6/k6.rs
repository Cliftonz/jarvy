use crate::tools::common::{InstallError, has, run};

/// Ensure `k6` is installed. We ignore the version hint for now and aim to
/// ensure the command is present on PATH.
pub fn ensure(_min_hint: &str) -> Result<(), InstallError> {
    if has("k6") {
        return Ok(());
    }
    install()
}

/// Registry adapter: allows tools::add("k6", version) to dispatch here
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
    // Homebrew formula name
    run("brew", &["install", "k6"])?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn install_linux() -> Result<(), InstallError> {
    if let Some(pm) = crate::tools::common::detect_linux_pm() {
        let _ = crate::tools::common::PkgOps::update(pm, true);
        // Many distros provide k6 directly as `k6` package or via community repos.
        // We attempt the generic name; environments without the package will surface CommandFailed.
        crate::tools::common::PkgOps::install(pm, "k6", true)
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
    // Official Grafana k6 package on winget
    run("winget", &["install", "-e", "--id", "Grafana.k6"])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_k6_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
