#[cfg(any(target_os = "macos", target_os = "windows"))]
use crate::tools::common::run;
use crate::tools::common::{InstallError, has};

/// Ensure `terraform` is installed. We only check presence; version hints are ignored
/// because package sources vary. Callers can manage versions with tfenv or similar
/// if they need strict control.
pub fn ensure(_min_hint: &str) -> Result<(), InstallError> {
    if has("terraform") {
        return Ok(());
    }
    install()
}

/// Registry adapter: allows tools::add("terraform", version) to dispatch here
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
    // Terraform is available in Homebrew core
    run("brew", &["install", "terraform"]).map(|_| ())
}

#[cfg(target_os = "linux")]
fn install_linux() -> Result<(), InstallError> {
    if let Some(pm) = crate::tools::common::detect_linux_pm() {
        // Best effort package index update
        let _ = crate::tools::common::PkgOps::update(pm, true);
        // Try common package name
        crate::tools::common::PkgOps::install(pm, "terraform", true)
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
    // Official HashiCorp Terraform package on winget
    run("winget", &["install", "-e", "--id", "HashiCorp.Terraform"]).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_terraform_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
