//! Scoop - Windows package manager, bootstrapped on demand.
//!
//! Some Windows tools ship no first-party winget or Chocolatey package
//! and rely solely on a Scoop bucket (`windows: { scoop: "..." }`, e.g.
//! Ory CLI). Historically that field was only consulted by the
//! maintenance freshness-checker (`maintenance::resolver`) — `jarvy
//! setup` itself never called `scoop install`, so a tool declaring only
//! `windows.scoop` silently fell through to `InstallError::Unsupported`.
//!
//! [`ensure_installed`] bootstraps Scoop via the official command
//! (<https://github.com/ScoopInstaller/Install>) — the same `curl | sh`
//! -> PowerShell bootstrap pattern already used for Chocolatey (see
//! `tools::chocolatey`). Every `windows.scoop` call site
//! (`ToolSpec::install_windows`) calls this instead of failing on a
//! missing `scoop`.
//!
//! Unlike Chocolatey's installer, Scoop's refuses to run under an
//! elevated PowerShell session by default (`Deny-Install 'Running the
//! installer as administrator is disabled by default...'`) unless the
//! documented `-RunAsAdmin` switch is passed. We pass it unconditionally
//! — it is a no-op when not elevated (`Test-IsAdministrator` is false,
//! so the deny-check never fires either way), and when elevated it only
//! changes behavior if the caller has also set `$env:SCOOP_GLOBAL` /
//! `$env:SCOOP_CACHE`, which jarvy's bootstrap invocation never does. So
//! passing it is strictly "don't abort on an elevated shell", not "opt
//! into a machine-wide install".

use std::sync::Mutex;

use crate::define_tool;
use crate::tools::common::{InstallContext, InstallError, forget_has, has, run};

/// The official one-line install command from the "For Admin" section of
/// <https://github.com/ScoopInstaller/Install>, unmodified — it already
/// passes `-RunAsAdmin` so the bootstrap doesn't abort when jarvy itself
/// is running elevated (see module docs for why this is safe
/// unconditionally).
const SCOOP_INSTALL_COMMAND: &str = "iex \"& {$(irm get.scoop.sh)} -RunAsAdmin\"";

/// Serializes the bootstrap. `jarvy setup` installs `custom_install`
/// tools on a rayon thread pool (see `commands::setup_cmd`), so two
/// tools that both fall back to Scoop can call `ensure_installed` at
/// the same instant — without this, both would see `scoop` missing and
/// both kick off the installer.
static BOOTSTRAP_LOCK: Mutex<()> = Mutex::new(());

/// Bootstrap Scoop if it isn't already on PATH. Safe to call
/// unconditionally, including concurrently — it's a no-op once `scoop`
/// is present, and only one caller ever runs the actual installer.
pub fn ensure_installed() -> Result<(), InstallError> {
    if has("scoop") {
        return Ok(());
    }

    let Ok(_guard) = BOOTSTRAP_LOCK.lock() else {
        return Err(InstallError::Prereq(
            "scoop bootstrap lock poisoned by an earlier panic".into(),
        ));
    };
    // Re-check now that we hold the lock: another thread may have
    // finished bootstrapping while we were waiting for it.
    if has("scoop") {
        return Ok(());
    }

    if !cfg!(target_os = "windows") {
        // Scoop only exists on Windows; every call site is already
        // behind a `windows.scoop` config slot, so reaching this on
        // another OS means the caller has nothing else to fall back to.
        return Err(InstallError::Prereq(
            "scoop not found. Install Scoop, then re-run.".into(),
        ));
    }

    println!("  Scoop not found - installing it first (required by this package)");
    run(
        "powershell",
        &[
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            SCOOP_INSTALL_COMMAND,
        ],
    )
    .map_err(|e| InstallError::Prereq(format!("Scoop bootstrap failed: {e}").into()))?;

    // The installer writes `scoop`'s directory to the user PATH via the
    // registry; this process won't see it until we re-read the registry
    // ourselves (see `windows::env_refresh`), and `has()` cached the
    // pre-bootstrap "missing" result above.
    crate::windows::env_refresh::refresh_current_process_path();
    forget_has("scoop");

    if has("scoop") {
        Ok(())
    } else {
        Err(InstallError::Prereq(
            "Scoop install script ran but `scoop` still isn't on PATH — \
             open a new shell and re-run `jarvy setup`"
                .into(),
        ))
    }
}

fn install_scoop(_min_hint: &str, _ctx: &InstallContext) -> Result<(), InstallError> {
    ensure_installed()
}

define_tool!(SCOOP, {
    command: "scoop",
    custom_install: install_scoop,
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scoop_registration_shape() {
        assert_eq!(SCOOP.command, "scoop");
        assert!(SCOOP.custom_install.is_some());
        assert!(
            SCOOP.windows.is_none(),
            "bootstrapped, not winget/choco/scoop-installed"
        );
    }
}
