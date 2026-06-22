//! Hardened binary-artifact installer for tools distributed as standalone
//! AppImages or tarballs from a vendor's download endpoint.
//!
//! Sibling of [`pinned_installer`](super::pinned_installer) which handles
//! the `curl | bash` script case. This module covers the binary case: pull
//! a pinned URL, sha256-verify the downloaded artifact, and only then
//! install it. A compromise of the vendor's CDN edge cannot land a
//! malicious binary on a user's machine because the sha256 mismatch
//! aborts before the artifact is moved into place.
//!
//! Two install shapes:
//!
//! - [`AppImagePin`] — single-file AppImage installed at `~/.local/bin/<name>`
//! - [`TarballAppPin`] — multi-file tarball extracted to
//!   `~/.local/share/jarvy/<name>/` with a symlink at
//!   `~/.local/bin/<name>` pointing at a binary inside the archive
//!
//! **To update a pin**: pick a new release, download the artifact, compute
//! its sha256, and update both URL and sha256 constants together. The
//! sha must be lowercase 64-char hex.

// Currently only consumed by Linux paths (cursor, jetbrains-toolbox).
// macOS / Windows builds reference brew + winget directly, so the helpers
// here would warn dead-code on those targets. Unit tests still need to run
// cross-platform — they're pure string-builder assertions — so we don't
// cfg-gate the module itself.
#![allow(dead_code)]

/// Pinned AppImage download — single self-contained executable.
pub struct AppImagePin<'a> {
    /// Display name used in log messages, refusal text, and the install path
    /// (e.g. `"cursor"` installs to `~/.local/bin/cursor`).
    pub name: &'a str,
    /// Direct HTTPS URL to the AppImage. MUST include a versioned commit /
    /// release hash in the path — no `latest` aliases that move underneath us.
    pub url: &'a str,
    /// Lowercase 64-char hex sha256 of the AppImage body.
    pub sha256: &'a str,
}

impl AppImagePin<'_> {
    /// Build a bash one-liner: download → sha verify → `install -m 0755`
    /// into `${XDG_BIN_HOME:-$HOME/.local/bin}/<name>`. The mismatch path
    /// aborts before the binary is moved into place, so a compromise of the
    /// vendor's edge cannot land RCE on a `jarvy setup`.
    pub fn shell_command(&self) -> String {
        format!(
            r#"set -euo pipefail
BIN_DIR="${{XDG_BIN_HOME:-$HOME/.local/bin}}"
mkdir -p "$BIN_DIR"
TMP=$(mktemp)
trap 'rm -f "$TMP"' EXIT
curl -fsSL '{url}' -o "$TMP"
ACTUAL=$(sha256sum "$TMP" | cut -d' ' -f1)
EXPECTED='{expected}'
if [ "$ACTUAL" != "$EXPECTED" ]; then
  printf 'jarvy: refusing to install %s; sha256 mismatch (got %s, want %s)\n' \
      '{name}' "$ACTUAL" "$EXPECTED" >&2
  exit 1
fi
install -m 0755 "$TMP" "$BIN_DIR/{name}"
printf 'jarvy: installed %s to %s/%s\n' '{name}' "$BIN_DIR" '{name}'
case ":$PATH:" in
  *":$BIN_DIR:"*) ;;
  *) printf 'jarvy: warning — %s is not on $PATH; add it to your shell rc to use %s\n' "$BIN_DIR" '{name}' >&2 ;;
esac
"#,
            url = self.url,
            expected = self.sha256,
            name = self.name,
        )
    }
}

/// Pinned tarball download — multi-file app where the binary needs sibling
/// files (libraries, JVM runtime, resources, etc.) to launch.
pub struct TarballAppPin<'a> {
    /// Display name and install symlink name (e.g. `"jetbrains-toolbox"`).
    pub name: &'a str,
    /// Direct HTTPS URL to the `.tar.gz` archive. Must include a versioned
    /// path component; no moving aliases.
    pub url: &'a str,
    /// Lowercase 64-char hex sha256 of the archive body.
    pub sha256: &'a str,
    /// Path to the launcher binary RELATIVE to the archive's top-level dir
    /// after `--strip-components=1` extraction. For example a tarball
    /// laying out as `foo-1.2.3/bin/foo` should pass `bin/foo` here.
    pub binary_relpath: &'a str,
}

impl TarballAppPin<'_> {
    /// Build a bash one-liner:
    ///
    /// 1. Download archive to a temp file
    /// 2. sha256-verify
    /// 3. Wipe any prior `~/.local/share/jarvy/<name>/` install
    /// 4. Extract with `--strip-components=1` so versioned root dirs flatten
    /// 5. Symlink `~/.local/bin/<name>` -> the launcher inside the extracted dir
    ///
    /// Idempotent: re-running the same install replaces a prior install
    /// cleanly. The sha-mismatch branch aborts before any extraction, so a
    /// compromised archive cannot leave half-extracted attack files on disk.
    pub fn shell_command(&self) -> String {
        format!(
            r#"set -euo pipefail
BIN_DIR="${{XDG_BIN_HOME:-$HOME/.local/bin}}"
SHARE_DIR="${{XDG_DATA_HOME:-$HOME/.local/share}}/jarvy/{name}"
mkdir -p "$BIN_DIR"
TMP=$(mktemp)
trap 'rm -f "$TMP"' EXIT
curl -fsSL '{url}' -o "$TMP"
ACTUAL=$(sha256sum "$TMP" | cut -d' ' -f1)
EXPECTED='{expected}'
if [ "$ACTUAL" != "$EXPECTED" ]; then
  printf 'jarvy: refusing to install %s; sha256 mismatch (got %s, want %s)\n' \
      '{name}' "$ACTUAL" "$EXPECTED" >&2
  exit 1
fi
rm -rf "$SHARE_DIR"
mkdir -p "$SHARE_DIR"
tar xzf "$TMP" --strip-components=1 -C "$SHARE_DIR"
ln -sfn "$SHARE_DIR/{binary_relpath}" "$BIN_DIR/{name}"
printf 'jarvy: installed %s to %s; symlink at %s/%s\n' \
    '{name}' "$SHARE_DIR" "$BIN_DIR" '{name}'
case ":$PATH:" in
  *":$BIN_DIR:"*) ;;
  *) printf 'jarvy: warning — %s is not on $PATH; add it to your shell rc to use %s\n' "$BIN_DIR" '{name}' >&2 ;;
esac
"#,
            url = self.url,
            expected = self.sha256,
            name = self.name,
            binary_relpath = self.binary_relpath,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> AppImagePin<'static> {
        AppImagePin {
            name: "demo",
            url: "https://example.com/sha/abc123/demo.AppImage",
            sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        }
    }

    fn tarball() -> TarballAppPin<'static> {
        TarballAppPin {
            name: "demo",
            url: "https://example.com/sha/abc123/demo.tar.gz",
            sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            binary_relpath: "bin/demo",
        }
    }

    #[test]
    fn appimage_command_embeds_url_and_hash() {
        let cmd = app().shell_command();
        assert!(cmd.contains("abc123/demo.AppImage"));
        assert!(cmd.contains(app().sha256));
        assert!(cmd.contains("'demo'"));
    }

    #[test]
    fn appimage_command_aborts_on_mismatch_before_install() {
        let cmd = app().shell_command();
        let exit_pos = cmd.find("exit 1").expect("exit 1 must appear");
        let install_pos = cmd.find("install -m 0755").expect("install must appear");
        assert!(
            exit_pos < install_pos,
            "sha256-mismatch refusal must short-circuit before install"
        );
    }

    #[test]
    fn tarball_command_embeds_url_hash_and_relpath() {
        let cmd = tarball().shell_command();
        assert!(cmd.contains("abc123/demo.tar.gz"));
        assert!(cmd.contains(tarball().sha256));
        assert!(cmd.contains("bin/demo"));
    }

    #[test]
    fn tarball_command_aborts_on_mismatch_before_extract() {
        let cmd = tarball().shell_command();
        let exit_pos = cmd.find("exit 1").expect("exit 1 must appear");
        let extract_pos = cmd.find("tar xzf").expect("tar xzf must appear");
        assert!(
            exit_pos < extract_pos,
            "sha256-mismatch refusal must short-circuit before extraction"
        );
    }

    #[test]
    fn tarball_command_wipes_prior_install_before_extract() {
        let cmd = tarball().shell_command();
        let wipe_pos = cmd
            .find("rm -rf \"$SHARE_DIR\"")
            .expect("wipe step must appear");
        let extract_pos = cmd.find("tar xzf").expect("tar xzf must appear");
        assert!(
            wipe_pos < extract_pos,
            "must wipe prior install before extracting new one for idempotency"
        );
    }

    #[test]
    fn commands_use_strict_bash() {
        for cmd in [app().shell_command(), tarball().shell_command()] {
            assert!(cmd.contains("set -euo pipefail"));
        }
    }

    #[test]
    fn commands_do_not_reference_moving_refs() {
        for cmd in [app().shell_command(), tarball().shell_command()] {
            assert!(!cmd.contains("/latest/"));
            assert!(!cmd.contains("/main/"));
            assert!(!cmd.contains("/HEAD/"));
        }
    }
}
