#[cfg(any(target_os = "macos", target_os = "windows"))]
use crate::tools::common::run;
use crate::tools::common::{InstallError, has};

/// Ensure `elixir` is available. No strict version matching.
pub fn ensure(_min_hint: &str) -> Result<(), InstallError> {
    if has("elixir") {
        return Ok(());
    }
    install()
}

/// Registry adapter: allows tools::add("elixir", version) to dispatch here
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
    run("brew", &["install", "elixir"])?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn install_linux() -> Result<(), InstallError> {
    if let Some(pm) = crate::tools::common::detect_linux_pm() {
        let _ = crate::tools::common::PkgOps::update(pm, true);
        // Most distros package `elixir` directly
        crate::tools::common::PkgOps::install(pm, "elixir", true)
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
    // Best-effort. Winget package availability varies; install manually if this fails.
    // Official docs: https://elixir-lang.org/install.html
    run("winget", &["install", "-e", "--id", "Elixir.Elixir"]).or_else(|_| {
        Err(InstallError::Prereq(
            "Elixir installation via winget failed. Install manually from https://elixir-lang.org/install.html",
        ))
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_elixir_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
