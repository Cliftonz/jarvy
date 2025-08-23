use super::common::{InstallError, run, has, require_any, cmd_satisfies};

/// Ensure Git is installed and at least roughly matches `min_hint`
/// (e.g., "2.40" → accepts 2.40.x+)
pub fn ensure(min_hint: &str) -> Result<(), InstallError> {
    if cmd_satisfies("git", min_hint) { return Ok(()); }
    install()
}

fn install() -> Result<(), InstallError> {
    #[cfg(target_os = "macos")]   { return install_macos(); }
    #[cfg(target_os = "linux")]   { return install_linux(); }
    #[cfg(target_os = "windows")] { return install_windows(); }
    #[allow(unreachable_code)]
    Err(InstallError::Unsupported)
}

#[cfg(target_os = "macos")]
fn install_macos() -> Result<(), InstallError> {
    if !has("brew") {
        return Err(InstallError::Prereq("Homebrew not found. Install https://brew.sh and re-run."));
    }
    run("brew", &["install", "git"])?; // modern Git via Homebrew
    Ok(())
}

#[cfg(target_os = "linux")]
fn install_linux() -> Result<(), InstallError> {
    let apt = require_any(&["apt", "apt-get"], "Need apt or apt-get on PATH for Debian/Ubuntu")?;
    run("sudo", &[apt, "update"])?;
    run("sudo", &[apt, "install", "-y", "git"])?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn install_windows() -> Result<(), InstallError> {
    if !has("winget") {
        return Err(InstallError::Prereq("winget not found. Install Windows Package Manager, then re-run."));
    }
    // Official Git for Windows package ID:
    run("winget", &["install", "-e", "--id", "Git.Git"])?; // exact ID avoids ambiguity
    Ok(())
}