//! `[windows]` configuration.
//!
//! Windows-specific setup knobs that mostly exist to fix "why does my
//! `.sh` script open in Notepad" — a paper cut for teams shipping any
//! bash-based tooling that Windows contributors need to run. Assumes
//! Git for Windows is installed (`bash.exe` under `C:\Program Files\
//! Git\bin\`); teams that use WSL exclusively don't need this block.

use serde::{Deserialize, Serialize};

/// `[windows]` block. Windows-only in behavior; parses on every OS so a
/// cross-platform team can commit one jarvy.toml. The setup phase
/// short-circuits on non-Windows with a `windows.phase_skipped` event.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WindowsConfig {
    /// Master enable. Default `true` — the block's presence implies
    /// enablement; users set `enabled = false` to declare-but-disable
    /// (matches `[git_hooks]` convention).
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Option B — add `.SH` to the user's PATHEXT env var so
    /// `myscript.sh` invokes bash from cmd / PowerShell without the
    /// caller needing to type `bash myscript.sh`. Default `false` —
    /// modifying PATHEXT is a durable env-var change, opt-in.
    ///
    /// Case-insensitive dedup: `.sh` already present in any casing
    /// leaves PATHEXT unchanged and emits `windows.pathext_unchanged`.
    #[serde(default)]
    pub sh_pathext: bool,

    /// Option C — set the Windows file association for `.sh` files.
    ///   `"off"`  = do not touch associations (default)
    ///   `"open"` = route the `"open"` verb to `bash.exe`, leaving the
    ///              `"edit"` verb alone so VS Code / Notepad++ can still
    ///              edit `.sh` files (Shift+right-click → Edit)
    ///
    /// Writes are per-user (HKCU) — no admin needed, no impact on
    /// other Windows accounts. Refuses if `bash.exe` cannot be located
    /// unless `bash_path_override` is set.
    #[serde(default = "default_sh_association")]
    pub sh_association: ShAssociationMode,

    /// Override the `bash.exe` path used for the `.sh` file association.
    /// When unset, jarvy looks for the standard Git for Windows install
    /// paths (`C:\Program Files\Git\bin\bash.exe`, then the WOW64 mirror).
    /// Set this explicitly for non-default install locations or if a
    /// team prefers `wsl.exe -e bash` as the launcher.
    ///
    /// SECURITY: the path is passed to `reg add` as-is. Refused if it
    /// contains characters that would break the registry command (`"`,
    /// newlines, or NUL) — see `validate_bash_path`.
    #[serde(default)]
    pub bash_path_override: Option<String>,

    /// Allow remote configs (`jarvy setup --from <url>`) to apply the
    /// Windows phase. Default `false`: a friendly-looking remote config
    /// cannot silently change a user's PATHEXT or file associations
    /// without an explicit opt-in in the source config. Mirrors the
    /// `[git_hooks] allow_remote` trust gate.
    #[serde(default)]
    pub allow_remote: bool,

    /// Origin tag set by `Config::mark_remote`; not serialized. Handlers
    /// consult this to enforce `allow_remote`. Mirrors `GitHooksConfig`.
    #[serde(skip)]
    pub origin: crate::ai_hooks::ConfigOrigin,
}

impl Default for WindowsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sh_pathext: false,
            sh_association: ShAssociationMode::Off,
            bash_path_override: None,
            allow_remote: false,
            origin: crate::ai_hooks::ConfigOrigin::Local,
        }
    }
}

impl crate::ai_hooks::HasOrigin for WindowsConfig {
    fn set_origin(&mut self, origin: crate::ai_hooks::ConfigOrigin) {
        self.origin = origin;
    }
}

/// Bounded modes for the `.sh` file association. String-typed (not
/// bool) because "off" is a real value users need to be able to write
/// to intentionally clear a prior team setting, AND because future
/// modes like `"open_and_edit"` (route both verbs) or `"wsl"` (launch
/// via `wsl.exe`) have a natural home here without shape churn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ShAssociationMode {
    /// Do not touch the `.sh` association. Default.
    Off,
    /// Route the `"open"` verb to `bash.exe`. Preserves `"edit"` verb
    /// so editors continue to open `.sh` for editing.
    Open,
}

fn default_true() -> bool {
    true
}

fn default_sh_association() -> ShAssociationMode {
    ShAssociationMode::Off
}

/// Validate a user-supplied `bash.exe` path before it flows into a
/// `reg add ... /d "..."` command. Returns the path unchanged when
/// safe, or an error string naming the offending character class.
///
/// Refused: NUL bytes (would truncate the arg on Windows), embedded
/// quotes (would break `reg add` quoting), newlines (same). Note we
/// do NOT check existence here — that's the executor's job, and
/// config-load is on non-Windows machines too.
pub fn validate_bash_path(path: &str) -> Result<&str, &'static str> {
    if path.contains('\0') {
        return Err("bash_path_override contains a NUL byte");
    }
    if path.contains('"') {
        return Err("bash_path_override contains a double-quote");
    }
    if path.contains('\n') || path.contains('\r') {
        return Err("bash_path_override contains a newline");
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_safe_and_opt_in() {
        let cfg = WindowsConfig::default();
        assert!(cfg.enabled);
        assert!(!cfg.sh_pathext, "PATHEXT edits are opt-in");
        assert_eq!(cfg.sh_association, ShAssociationMode::Off);
        assert!(cfg.bash_path_override.is_none());
        assert!(!cfg.allow_remote);
    }

    #[test]
    fn defaults_match_serde_empty() {
        let cfg: WindowsConfig = toml::from_str("").unwrap();
        let default = WindowsConfig::default();
        assert_eq!(cfg.enabled, default.enabled);
        assert_eq!(cfg.sh_pathext, default.sh_pathext);
        assert_eq!(cfg.sh_association, default.sh_association);
        assert_eq!(cfg.allow_remote, default.allow_remote);
    }

    #[test]
    fn parses_both_pathext_and_association_open() {
        let toml_str = r#"
sh_pathext = true
sh_association = "open"
"#;
        let cfg: WindowsConfig = toml::from_str(toml_str).unwrap();
        assert!(cfg.sh_pathext);
        assert_eq!(cfg.sh_association, ShAssociationMode::Open);
    }

    #[test]
    fn parses_off_explicitly() {
        let cfg: WindowsConfig = toml::from_str(r#"sh_association = "off""#).unwrap();
        assert_eq!(cfg.sh_association, ShAssociationMode::Off);
    }

    #[test]
    fn parses_bash_path_override() {
        let cfg: WindowsConfig =
            toml::from_str(r#"bash_path_override = "C:\\Git\\bin\\bash.exe""#).unwrap();
        assert_eq!(
            cfg.bash_path_override.as_deref(),
            Some("C:\\Git\\bin\\bash.exe")
        );
    }

    #[test]
    fn validate_accepts_typical_windows_paths() {
        assert!(validate_bash_path("C:\\Program Files\\Git\\bin\\bash.exe").is_ok());
        assert!(validate_bash_path("D:\\tools\\git\\bin\\bash.exe").is_ok());
    }

    #[test]
    fn validate_refuses_quote() {
        assert!(validate_bash_path("C:\\Program Files\\Git\\bin\\bash\".exe").is_err());
    }

    #[test]
    fn validate_refuses_nul() {
        assert!(validate_bash_path("C:\\bash.exe\0evil").is_err());
    }

    #[test]
    fn validate_refuses_newline() {
        assert!(validate_bash_path("C:\\bash.exe\nreg add HKLM ...").is_err());
    }

    #[test]
    fn origin_defaults_to_local() {
        let cfg: WindowsConfig = toml::from_str("").unwrap();
        assert_eq!(cfg.origin, crate::ai_hooks::ConfigOrigin::Local);
    }
}
