//! Ory CLI — manage Ory Network projects and the Ory open-source stack
//! (Kratos identity, Hydra OAuth2/OIDC, Keto permissions, Oathkeeper).
//!
//! Verified 2026-08-28 against <https://www.ory.com/docs/guides/cli/installation>:
//!
//! - macOS: `brew install ory/tap/cli` — a two-slash tap formula, so
//!   jarvy's `install_macos()` auto-taps `ory/tap` before installing.
//! - Windows: Ory ships no first-party winget or Chocolatey package,
//!   only a Scoop bucket (`scoop bucket add ory ...` + `scoop install
//!   ory`). Declared via `windows: { scoop: "ory" }`.
//! - Linux: Ory ships no apt/dnf/pacman package and no Linuxbrew tap —
//!   the only documented method is `ory/meta`'s `install.sh` script.
//!   That repository has no tags/releases to pin to (unlike nvm-sh,
//!   which this mirrors via a pinned tag URL), so the fetch is pinned
//!   to commit `1140624a363011e737375f796ea4fdbaac96cbb` — the most
//!   recent commit touching `install.sh` as of this verification date
//!   (checked via the GitHub API) — rather than the mutable `master`
//!   branch. The script performs its own SHA-256 checksum verification
//!   of the downloaded release tarball against `checksums.txt` before
//!   installing, so this pin only closes the "script content itself
//!   changes underneath us" gap, not binary integrity.

use crate::define_tool;
use crate::tools::common::{InstallContext, InstallError};
#[cfg(target_os = "linux")]
use crate::tools::common::{has, run};

/// Commit in `ory/meta` last touching `install.sh`, used in place of a
/// release tag (the repo has none) to keep the script fetch immutable.
#[cfg(target_os = "linux")]
const ORY_META_INSTALL_SH_COMMIT: &str = "1140624a363011e737375f796ea4fdbaac96cbb";

#[cfg(target_os = "linux")]
fn install_ory_linux() -> Result<(), InstallError> {
    if !has("curl") {
        return Err(InstallError::Prereq(
            "curl is required to install the Ory CLI".into(),
        ));
    }
    run(
        "sh",
        &[
            "-c",
            &format!(
                r#"set -eu
BIN_DIR="${{XDG_BIN_HOME:-$HOME/.local/bin}}"
mkdir -p "$BIN_DIR"
curl -fsSL https://raw.githubusercontent.com/ory/meta/{ORY_META_INSTALL_SH_COMMIT}/install.sh | bash -s -- -b "$BIN_DIR" ory"#
            ),
        ],
    )?;
    Ok(())
}

fn install_ory(_min_hint: &str, _ctx: &InstallContext) -> Result<(), InstallError> {
    #[cfg(target_os = "linux")]
    {
        install_ory_linux()
    }
    #[cfg(not(target_os = "linux"))]
    {
        ORY.install_platform()
    }
}

define_tool!(ORY, {
    command: "ory",
    macos: { brew: "ory/tap/cli" },
    windows: { scoop: "ory" },
    custom_install: install_ory,
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ory_registration_shape() {
        assert_eq!(ORY.command, "ory");
        let mac = ORY.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("ory/tap/cli"));
        let win = ORY.windows.expect("must support Windows");
        assert_eq!(win.scoop, Some("ory"));
        assert!(ORY.custom_install.is_some(), "Linux curl|bash installer");
        assert!(
            ORY.linux.is_none(),
            "Linux install routes through custom_install, not a package manager"
        );
    }
}
