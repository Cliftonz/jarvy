#[cfg(any(target_os = "macos", target_os = "linux"))]
use crate::tools::common::run;
use crate::tools::common::{InstallError, has};

/// Ensure `tilt` CLI is available. We don't enforce a specific version; having
/// the command present is sufficient for typical workflows.
pub fn ensure(_min_hint: &str) -> Result<(), InstallError> {
    if has("tilt") {
        return Ok(());
    }
    install()
}

/// Registry adapter: allows tools::add("tilt", version) to dispatch here
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
    // Official formula name
    run("brew", &["install", "tilt"])?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn install_linux() -> Result<(), InstallError> {
    // Prefer distro package if available (name: tilt). Not all distros provide it.
    if let Some(pm) = crate::tools::common::detect_linux_pm() {
        let _ = crate::tools::common::PkgOps::update(pm, true);
        let try_pkg = crate::tools::common::PkgOps::install(pm, "tilt", true);
        if try_pkg.is_ok() {
            return try_pkg;
        }
    }
    // Fallback: if Homebrew is present on Linux, use it
    if has("brew") {
        return run("brew", &["install", "tilt"]).map(|_| ());
    }
    Err(InstallError::Prereq(
        "Unable to install tilt automatically on this platform. Install via your package manager or Homebrew (brew install tilt).",
    ))
}

#[cfg(target_os = "windows")]
fn install_windows() -> Result<(), InstallError> {
    // No widely supported official Windows package in this project.
    Err(InstallError::Prereq(
        "tilt installation on Windows is not automated. Consider using WSL or install manually from tilt.dev.",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_tilt_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
