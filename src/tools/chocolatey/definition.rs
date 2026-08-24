//! Chocolatey - Windows package manager, bootstrapped on demand.
//!
//! A growing set of Windows tools in this registry (syft, grype,
//! ansible, dbmate, git-town, pgcli, watchman, ...) ship no first-party
//! winget manifest and rely solely on a Chocolatey package
//! (`windows: { choco: "..." }`). Historically that meant `jarvy setup`
//! failed outright with `Prereq("chocolatey not found...")` on any
//! machine that hadn't already installed Chocolatey by hand — jarvy's
//! whole pitch is "provisions tools via native package managers", so
//! requiring a *second*, unrelated package manager to be pre-installed
//! by the user defeats the point.
//!
//! [`ensure_installed`] bootstraps Chocolatey via the official
//! individual-install command (<https://chocolatey.org/install#individual>)
//! — the Windows-PowerShell equivalent of the `curl | sh` bootstrap
//! already used for nvm/rustup (see `tools::nvm`, `tools::rust`). Every
//! `windows.choco` call site (`ToolSpec::install_windows`,
//! `PackageManager::batch_install_choco`, plugin tools, the Java
//! installer's Chocolatey route) calls this instead of failing on a
//! missing `choco`.

use std::sync::Mutex;

use crate::define_tool;
use crate::tools::common::{InstallContext, InstallError, forget_has, has, run};

/// The official individual-install command from
/// <https://chocolatey.org/install#individual>, unmodified.
const CHOCOLATEY_INSTALL_COMMAND: &str = "Set-ExecutionPolicy Bypass -Scope Process -Force; \
     [System.Net.ServicePointManager]::SecurityProtocol = [System.Net.ServicePointManager]::SecurityProtocol -bor 3072; \
     iex ((New-Object System.Net.WebClient).DownloadString('https://community.chocolatey.org/install.ps1'))";

/// Serializes the bootstrap. `jarvy setup` installs `custom_install`
/// tools on a rayon thread pool (see `commands::setup_cmd`), so two
/// tools that both fall back to Chocolatey (syft + grype, say) can call
/// `ensure_installed` at the same instant — without this, both would
/// see `choco` missing and both kick off the installer.
static BOOTSTRAP_LOCK: Mutex<()> = Mutex::new(());

/// Bootstrap Chocolatey if it isn't already on PATH. Safe to call
/// unconditionally, including concurrently — it's a no-op once `choco`
/// is present, and only one caller ever runs the actual installer.
pub fn ensure_installed() -> Result<(), InstallError> {
    if has("choco") {
        return Ok(());
    }

    let Ok(_guard) = BOOTSTRAP_LOCK.lock() else {
        return Err(InstallError::Prereq(
            "chocolatey bootstrap lock poisoned by an earlier panic".into(),
        ));
    };
    // Re-check now that we hold the lock: another thread may have
    // finished bootstrapping while we were waiting for it.
    if has("choco") {
        return Ok(());
    }

    if !cfg!(target_os = "windows") {
        // Chocolatey only exists on Windows; every call site is already
        // behind a `windows.choco` config slot, so reaching this on
        // another OS means the caller has nothing else to fall back to.
        return Err(InstallError::Prereq(
            "chocolatey not found. Install Chocolatey, then re-run.".into(),
        ));
    }

    println!("  Chocolatey not found - installing it first (required by this package)");
    run(
        "powershell",
        &[
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            CHOCOLATEY_INSTALL_COMMAND,
        ],
    )
    .map_err(|e| InstallError::Prereq(format!("Chocolatey bootstrap failed: {e}").into()))?;

    // The installer writes `choco`'s directory to the machine PATH via
    // the registry; this process won't see it until we re-read the
    // registry ourselves (see `windows::env_refresh`), and `has()`
    // cached the pre-bootstrap "missing" result above.
    crate::windows::env_refresh::refresh_current_process_path();
    forget_has("choco");

    if has("choco") {
        Ok(())
    } else {
        Err(InstallError::Prereq(
            "Chocolatey install script ran but `choco` still isn't on PATH — \
             open a new shell and re-run `jarvy setup`"
                .into(),
        ))
    }
}

fn install_chocolatey(_min_hint: &str, _ctx: &InstallContext) -> Result<(), InstallError> {
    ensure_installed()
}

define_tool!(CHOCOLATEY, {
    command: "choco",
    custom_install: install_chocolatey,
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chocolatey_registration_shape() {
        assert_eq!(CHOCOLATEY.command, "choco");
        assert!(CHOCOLATEY.custom_install.is_some());
        assert!(
            CHOCOLATEY.windows.is_none(),
            "bootstrapped, not winget/choco-installed"
        );
    }
}
