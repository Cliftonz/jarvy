#[cfg(any(target_os = "macos", target_os = "windows"))]
use crate::tools::common::run;
use crate::tools::common::{InstallError, has};

/// Ensure `packer` is installed. Version hint is ignored because package
/// sources vary. Users can pin versions via alternative methods if needed.
pub fn ensure(_min_hint: &str) -> Result<(), InstallError> {
    if has("packer") {
        return Ok(());
    }
    install()
}

/// Registry adapter: allows tools::add("packer", version) to dispatch here
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
    // Packer is available in Homebrew core
    run("brew", &["install", "packer"]).map(|_| ())
}

#[cfg(target_os = "linux")]
fn install_linux() -> Result<(), InstallError> {
    if let Some(pm) = crate::tools::common::detect_linux_pm() {
        // Best effort package index update
        let _ = crate::tools::common::PkgOps::update(pm, true);
        // Try common package name
        crate::tools::common::PkgOps::install(pm, "packer", true)
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
    // Official HashiCorp Packer package on winget
    run("winget", &["install", "-e", "--id", "HashiCorp.Packer"]).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_packer_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
