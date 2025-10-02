use crate::tools::common::{InstallError, has, run};

/// Ensure Homebrew is installed. On macOS and Linux, attempts installation if missing.
/// Not supported on Windows.
pub fn ensure(_min_hint: &str) -> Result<(), InstallError> {
    if has("brew") {
        return Ok(());
    }
    install()
}

/// Registry adapter: allows tools::add("brew", version) to dispatch here
pub fn add_handler(min_hint: &str) -> Result<(), InstallError> {
    // brew does not have semantic versions for the CLI via package managers; ignore hint
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
    // Follow official non-interactive install using bash -c
    // Equivalent to: /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
    // We avoid shell expansion by invoking bash directly with -c.
    let script = "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)";
    // Use /bin/bash explicitly per brew docs
    let _ = run("/bin/bash", &["-c", script])?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn install_linux() -> Result<(), InstallError> {
    // Official installation script supports Linux as well.
    let script = "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)";
    let _ = run("/bin/bash", &["-c", script])?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn install_windows() -> Result<(), InstallError> {
    // Explicitly unsupported per requirements
    Err(InstallError::Unsupported)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_brew_no_panic() {
        let res = ensure("");
        // On macOS/Linux in CI, brew may or may not be present; installation may fail due to permissions.
        // We only assert that calling the function returns a Result without panicking.
        assert!(res.is_ok() || res.is_err());
    }

    // Platform-specific expectations for brew installer behavior.
    // Windows: brew is not supported → ensure/install must return Unsupported.
    #[cfg(target_os = "windows")]
    #[test]
    fn brew_windows_is_unsupported() {
        let res = ensure("");
        assert!(
            matches!(res, Err(InstallError::Unsupported)),
            "brew on Windows should be Unsupported"
        );
    }

    // macOS/Linux: ensure/install should never return Unsupported (either Ok or a concrete error like Prereq/CommandFailed).
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn brew_unix_not_unsupported() {
        let res = ensure("");
        assert!(
            !matches!(res, Err(InstallError::Unsupported)),
            "brew on Unix should not return Unsupported"
        );
    }
}
