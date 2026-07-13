#[cfg(target_os = "windows")]
use crate::tools::common::has;
use crate::tools::common::{InstallError, run};
use std::path::PathBuf;

/// Shell-independent probe for an existing nvm installation.
///
/// nvm is a shell FUNCTION sourced from `$NVM_DIR/nvm.sh`, not a binary:
/// `command -v nvm` only succeeds in a shell that already sourced it, and
/// `has("nvm")` (PATH lookup) never does. The previous probe ran
/// `bash -lc 'command -v nvm'` — a bash *login* shell — but the nvm
/// installer picks its profile file from `$SHELL`, so on macOS (zsh) the
/// init lines land in `~/.zshrc`, which `bash -lc` never sources. Result:
/// the probe could not see the install it had just performed and every
/// `jarvy setup` re-ran the installer.
///
/// The canonical marker is the same one nvm's own init guard uses: a
/// non-empty `$NVM_DIR/nvm.sh` (default `~/.nvm/nvm.sh`).
///
/// On Windows, nvm-windows IS a real binary, so a PATH lookup is correct.
pub fn is_installed() -> bool {
    #[cfg(target_os = "windows")]
    {
        has("nvm")
    }
    #[cfg(not(target_os = "windows"))]
    {
        nvm_sh_path(std::env::var_os("NVM_DIR"), dirs::home_dir())
            .and_then(|p| std::fs::metadata(p).ok())
            .map(|m| m.is_file() && m.len() > 0)
            .unwrap_or(false)
    }
}

/// Resolve the expected location of `nvm.sh`: `$NVM_DIR` when set and
/// non-empty, else `~/.nvm`. Pure so the precedence is unit-testable
/// without mutating process env.
fn nvm_sh_path(nvm_dir: Option<std::ffi::OsString>, home: Option<PathBuf>) -> Option<PathBuf> {
    let dir = nvm_dir
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .or_else(|| home.map(|h| h.join(".nvm")))?;
    Some(dir.join("nvm.sh"))
}

pub fn ensure(_min_hint: &str) -> Result<(), InstallError> {
    if is_installed() {
        return Ok(());
    }
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        install_posix()
    }
    #[cfg(target_os = "windows")]
    {
        install_windows()
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        Err(InstallError::Unsupported)
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn install_posix() -> Result<(), InstallError> {
    // Official installer script from nvm-sh
    // https://github.com/nvm-sh/nvm
    run(
        "bash",
        &[
            "-lc",
            r#"curl -fsSL https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.7/install.sh | bash"#,
        ],
    )?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn install_windows() -> Result<(), InstallError> {
    if !has("winget") {
        return Err(InstallError::Prereq(
            "winget not found. Install Windows Package Manager, then re-run.",
        ));
    }
    // NVM for Windows official package ID
    run(
        "winget",
        &["install", "-e", "--id", "CoreyButler.NVMforWindows"],
    )?;
    Ok(())
}

/// Registry adapter: allows tools::add("nvm", version) to dispatch here
pub fn add_handler(min_hint: &str) -> Result<(), InstallError> {
    // nvm installation does not strictly adhere to semantic versions; installer installs latest stable.
    // We ignore provided hint for now and just ensure it's present.
    let _ = min_hint;
    ensure("")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    #[test]
    fn nvm_dir_env_wins_over_home() {
        let p = nvm_sh_path(
            Some(OsString::from("/custom/nvm")),
            Some(PathBuf::from("/home/u")),
        )
        .unwrap();
        assert_eq!(p, PathBuf::from("/custom/nvm/nvm.sh"));
    }

    #[test]
    fn empty_nvm_dir_falls_back_to_home() {
        let p = nvm_sh_path(Some(OsString::new()), Some(PathBuf::from("/home/u"))).unwrap();
        assert_eq!(p, PathBuf::from("/home/u/.nvm/nvm.sh"));
    }

    #[test]
    fn no_env_no_home_is_none() {
        assert!(nvm_sh_path(None, None).is_none());
    }

    #[test]
    fn marker_probe_detects_real_file() {
        let tmp = std::env::temp_dir().join(format!("jarvy-nvm-test-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let marker = tmp.join("nvm.sh");
        std::fs::write(&marker, "# nvm init\n").unwrap();
        let p = nvm_sh_path(Some(tmp.clone().into_os_string()), None).unwrap();
        let meta = std::fs::metadata(&p).unwrap();
        assert!(meta.is_file() && meta.len() > 0);
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
