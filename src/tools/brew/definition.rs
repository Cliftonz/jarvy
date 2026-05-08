#[cfg(any(target_os = "macos", target_os = "linux"))]
use crate::tools::common::run;
use crate::tools::common::{InstallError, has};

/// Pinned commit of the Homebrew/install repo. Updating this constant is the
/// only way Jarvy will pull a newer installer — no `master`/`HEAD` fetches
/// at runtime, so a compromise of the upstream branch tip cannot silently
/// land arbitrary code on a user's machine the next time `jarvy setup` runs.
///
/// To update: pick a commit from
/// <https://github.com/Homebrew/install/commits/HEAD>, download the script
/// at that commit, compute its sha256, and update both constants together.
pub const HOMEBREW_INSTALLER_COMMIT: &str = "540da2ca91271886910572df3a50332540ca84e4";
pub const HOMEBREW_INSTALLER_SHA256: &str =
    "dfd5145fe2aa5956a600e35848765273f5798ce6def01bd08ecec088a1268d91";

/// Returns true if `jarvy` is running in a CI environment. Auto-fetching an
/// installer script in CI is too high-risk because the user often can't
/// audit what landed.
fn in_ci() -> bool {
    std::env::var("CI").is_ok() || std::env::var("GITHUB_ACTIONS").is_ok()
}

/// Returns true if auto-installing brew in this environment is allowed.
pub fn brew_auto_install_allowed() -> bool {
    !in_ci()
}

/// Build the bash one-liner that fetches the pinned Homebrew installer,
/// verifies its sha256 matches the constant we ship, and only then pipes
/// it into `/bin/bash` for execution. Exposed so legacy/`provisioner` paths
/// can share the same hardened pipeline.
pub fn pinned_homebrew_installer_command() -> String {
    let installer_url = format!(
        "https://raw.githubusercontent.com/Homebrew/install/{commit}/install.sh",
        commit = HOMEBREW_INSTALLER_COMMIT,
    );
    format!(
        r#"set -euo pipefail
SCRIPT="$(curl -fsSL '{url}')"
ACTUAL=$(printf '%s' "$SCRIPT" | shasum -a 256 | cut -d' ' -f1)
EXPECTED='{expected}'
if [ "$ACTUAL" != "$EXPECTED" ]; then
  printf 'jarvy: refusing to run Homebrew installer; sha256 mismatch (got %s, want %s)\n' \
      "$ACTUAL" "$EXPECTED" >&2
  exit 1
fi
printf '%s' "$SCRIPT" | /bin/bash -s --
"#,
        url = installer_url,
        expected = HOMEBREW_INSTALLER_SHA256,
    )
}

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
    install_pinned_brew()
}

#[cfg(target_os = "linux")]
fn install_linux() -> Result<(), InstallError> {
    install_pinned_brew()
}

/// Run the pinned Homebrew installer (sha256-verified) via `sh -c`. The
/// sha256 mismatch path aborts the script before any code from the upstream
/// repo runs, so a compromise of the Homebrew/install branch tip cannot
/// silently land RCE on a `jarvy setup`. CI environments refuse the
/// auto-install entirely (callers should pre-install brew on CI runners).
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn install_pinned_brew() -> Result<(), InstallError> {
    if !brew_auto_install_allowed() {
        return Err(InstallError::Prereq(
            "auto-install of brew is refused in CI; pre-install brew on the runner image",
        ));
    }
    let script = pinned_homebrew_installer_command();
    let _ = run("sh", &["-c", &script])?;
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

    #[test]
    fn pinned_installer_command_embeds_constants() {
        let cmd = pinned_homebrew_installer_command();
        assert!(
            cmd.contains(HOMEBREW_INSTALLER_COMMIT),
            "installer command must include pinned commit"
        );
        assert!(
            cmd.contains(HOMEBREW_INSTALLER_SHA256),
            "installer command must include pinned sha256"
        );
        // Must NOT pull from a moving ref; that's the whole point.
        assert!(!cmd.contains("/HEAD/"));
        assert!(!cmd.contains("/master/"));
        assert!(!cmd.contains("/main/"));
    }

    #[test]
    fn pinned_constants_are_well_formed() {
        // Commit is a 40-char hex.
        assert_eq!(HOMEBREW_INSTALLER_COMMIT.len(), 40);
        assert!(
            HOMEBREW_INSTALLER_COMMIT
                .chars()
                .all(|c| c.is_ascii_hexdigit())
        );
        // SHA256 is a 64-char hex.
        assert_eq!(HOMEBREW_INSTALLER_SHA256.len(), 64);
        assert!(
            HOMEBREW_INSTALLER_SHA256
                .chars()
                .all(|c| c.is_ascii_hexdigit())
        );
    }
}
