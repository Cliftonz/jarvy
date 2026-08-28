//! `[windows]` configuration.
//!
//! Windows-specific setup knobs that mostly exist to fix "why does my
//! `.sh` script open in Notepad" — a paper cut for teams shipping any
//! bash-based tooling that Windows contributors need to run. Assumes
//! Git for Windows is installed (`bash.exe` under `C:\Program Files\
//! Git\bin\`); teams that use WSL exclusively don't need this block.

// WSL sub-block helpers (translator, validators, effective_*) are
// consumed by the Windows exec code and by cross-platform integration
// tests. On non-Windows builds without the integration tests compiled
// in, rustc would flag them as dead. Every WSL helper has test
// coverage in `tests/wsl_tool_integration.rs`.
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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
    ///
    /// The same gate governs `[windows.wsl]`; a remote config carrying
    /// the WSL sub-block still requires `allow_remote = true` here.
    #[serde(default)]
    pub allow_remote: bool,

    /// `[windows.wsl]` sub-block — bootstraps a named WSL2 distro,
    /// optionally installs jarvy inside it, and (when `run_setup =
    /// true`) delegates the outer `jarvy setup` into the distro on the
    /// translated project path. Opt-in; the tool is inert until
    /// `enabled = true`. See `src/tools/wsl/`.
    #[serde(default)]
    pub wsl: Option<WslConfig>,

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
            wsl: None,
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

/// How CLIs inside a WSL distro launch a browser on the Windows host.
/// String-typed for the same reason as [`ShAssociationMode`]: `"off"`
/// is a real value users write to intentionally clear a prior team
/// setting, and future modes have a natural home here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserLauncherMode {
    /// Do not touch `$BROWSER` inside the distro. Default.
    Off,
    /// Install the `wslu` apt package and set `$BROWSER=wslview`.
    /// Only available on distros that carry `wslu` in an official
    /// repo (Ubuntu's `universe`); refused for `distro = "Debian"`
    /// at config-load via [`WslConfig::validate`].
    Wslu,
    /// Set `$BROWSER` to a `cmd.exe /c start` shim. No package
    /// install; works on any distro.
    CmdShim,
}

impl BrowserLauncherMode {
    /// Bounded label for telemetry. Never called for `Off` — callers
    /// early-return before needing a label for that variant.
    pub fn as_label(self) -> &'static str {
        match self {
            BrowserLauncherMode::Off => "off",
            BrowserLauncherMode::Wslu => "wslu",
            BrowserLauncherMode::CmdShim => "cmd_shim",
        }
    }
}

fn default_browser_launcher() -> BrowserLauncherMode {
    BrowserLauncherMode::Off
}

/// `[windows.wsl]` sub-block. Bootstraps a named WSL2 distro on
/// Windows, optionally installs jarvy inside it, and optionally
/// delegates the outer `jarvy setup` into the distro on the translated
/// project path.
///
/// Parses on every OS so a cross-platform team can commit one
/// jarvy.toml; the tool short-circuits on non-Windows. Opt-in via
/// `enabled = true`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WslConfig {
    /// Opt-in gate. Default `false` — the tool is inert until this is
    /// explicitly true. Even when true, `auto_bootstrap = false` still
    /// refuses to run `wsl --install` and prints the manual command.
    #[serde(default)]
    pub enabled: bool,

    /// Base image slug. v1 supports `"Ubuntu"` and `"Debian"`; other
    /// slugs are refused at config-load with a clear error.
    #[serde(default = "default_distro")]
    pub distro: String,

    /// Instance name (`wsl -l -q` shows this). Defaults to `distro`
    /// verbatim, which will collide with any existing bare Ubuntu /
    /// Debian on the machine — a feature for users who want jarvy to
    /// adopt their existing distro, a bug for users who want isolation.
    /// Set explicitly (e.g. `"jarvy-ubuntu"`) for isolation.
    ///
    /// Validated against `validate_distro_name` at load: bounded
    /// charset, length cap, refuses Docker Desktop's reserved names.
    #[serde(default)]
    pub name: Option<String>,

    /// If `true` and Store-WSL + the named distro are both absent,
    /// jarvy prints the exact elevated `wsl --install` command for the
    /// user to run. Jarvy NEVER triggers UAC itself. When `true` AND
    /// the process is already elevated, jarvy runs the install.
    #[serde(default)]
    pub auto_bootstrap: bool,

    /// Where the distro rootfs vhd lives on disk. `"auto"` resolves
    /// to `%LOCALAPPDATA%\jarvy\wsl\<name>`. Explicit paths must be
    /// absolute (no UNC, no relative, no NUL / quotes / newlines).
    #[serde(default = "default_install_location")]
    pub install_location: String,

    /// After the distro is present, curl-pipe the jarvy install script
    /// inside it (skipped if `jarvy` is already on PATH inside). Default
    /// `true` — teams opting into the WSL bridge almost always want the
    /// inner jarvy to install the actual project tools.
    #[serde(default = "default_true")]
    pub install_jarvy: bool,

    /// Channel passed to the inner install script. Bounded: `"stable"`
    /// / `"beta"` / `"nightly"`. Unknown channels are refused at load.
    #[serde(default = "default_channel")]
    pub jarvy_channel: String,

    /// When the outer invocation is `jarvy setup`, re-exec
    /// `wsl -d <name> -- bash -lc "cd '/mnt/c/...' && jarvy setup"` in
    /// the translated project path after bootstrap. Opt-in because it
    /// radically changes the outer setup's behavior (tools install
    /// inside the distro, not on Windows).
    #[serde(default)]
    pub run_setup: bool,

    /// How CLIs inside the distro (e.g. `gh auth login`) launch a
    /// browser on the Windows host:
    ///   `"off"`      = do not touch `$BROWSER` inside the distro (default)
    ///   `"wslu"`     = install the `wslu` apt package and set
    ///                  `$BROWSER=wslview`
    ///   `"cmd_shim"` = set `$BROWSER` to a `cmd.exe /c start` shim,
    ///                  no package install
    ///
    /// `"wslu"` is only in Ubuntu's official `universe` repo, not in
    /// Debian's official repos — `validate()` refuses the combination
    /// of `browser_launcher = "wslu"` with `distro = "Debian"`.
    #[serde(default = "default_browser_launcher")]
    pub browser_launcher: BrowserLauncherMode,
}

impl Default for WslConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            distro: default_distro(),
            name: None,
            auto_bootstrap: false,
            install_location: default_install_location(),
            install_jarvy: true,
            jarvy_channel: default_channel(),
            run_setup: false,
            browser_launcher: BrowserLauncherMode::Off,
        }
    }
}

impl WslConfig {
    /// Effective instance name: user's `name` if set, else `distro`.
    pub fn effective_name(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.distro)
    }

    /// Effective on-disk install location, resolving `"auto"` to
    /// `%LOCALAPPDATA%\jarvy\wsl\<name>` when the LocalAppData env
    /// resolver is available. On non-Windows or when `%LOCALAPPDATA%`
    /// is unset, returns `~/.jarvy/wsl/<name>` as a portable fallback
    /// (used by tests; the executor is Windows-only).
    pub fn effective_install_location(&self) -> PathBuf {
        if self.install_location != "auto" {
            return PathBuf::from(&self.install_location);
        }
        let name = self.effective_name();
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            let mut p = PathBuf::from(local);
            p.push("jarvy");
            p.push("wsl");
            p.push(name);
            return p;
        }
        if let Some(mut p) = dirs::home_dir() {
            p.push(".jarvy");
            p.push("wsl");
            p.push(name);
            return p;
        }
        PathBuf::from(name)
    }

    /// Validate the whole block at config-load time. Called after
    /// serde deserialization from the [`WindowsConfig`] parent so users
    /// get a clean parse error on any invalid value.
    pub fn validate(&self) -> Result<(), String> {
        validate_distro_slug(&self.distro)?;
        if self.browser_launcher == BrowserLauncherMode::Wslu
            && self.distro.eq_ignore_ascii_case("Debian")
        {
            return Err(
                "[windows.wsl] browser_launcher = \"wslu\" is not available on Debian (wslu is not in Debian's official repos); use browser_launcher = \"cmd_shim\" instead"
                    .to_string(),
            );
        }
        if let Some(name) = self.name.as_deref() {
            validate_distro_name(name)?;
        } else {
            // Default `name = distro` still has to pass reserved-name /
            // charset checks — a distro slug is user-supplied via TOML
            // even before it becomes an instance name.
            validate_distro_name(&self.distro)?;
        }
        validate_install_location(&self.install_location)?;
        validate_channel(&self.jarvy_channel)?;
        Ok(())
    }
}

/// v1 base-distro allowlist. Every entry has a stable rootfs URL that
/// `wsl --import` can consume when the caller's Store-WSL doesn't yet
/// support `wsl --install --name`.
const SUPPORTED_BASE_DISTROS: &[&str] = &["Ubuntu", "Debian"];

/// Instance names collision-refused at load: `docker-desktop` and
/// `docker-desktop-data` are the WSL distros Docker Desktop registers
/// for its own runtime; overwriting either would break Docker Desktop
/// on the machine.
const RESERVED_DISTRO_NAMES: &[&str] = &["docker-desktop", "docker-desktop-data"];

const BOUNDED_JARVY_CHANNELS: &[&str] = &["stable", "beta", "nightly"];

fn default_distro() -> String {
    "Ubuntu".to_string()
}

fn default_install_location() -> String {
    "auto".to_string()
}

fn default_channel() -> String {
    "stable".to_string()
}

/// Reject unsupported base-distro slugs at config load. v1 supports
/// only `"Ubuntu"` and `"Debian"`; adding more requires a pinned
/// rootfs URL in `src/tools/wsl/rootfs.rs`.
pub fn validate_distro_slug(slug: &str) -> Result<(), String> {
    if SUPPORTED_BASE_DISTROS
        .iter()
        .any(|s| s.eq_ignore_ascii_case(slug))
    {
        Ok(())
    } else {
        Err(format!(
            "[windows.wsl] distro = \"{slug}\" is not supported in v1. Supported: {}",
            SUPPORTED_BASE_DISTROS.join(", ")
        ))
    }
}

/// Distro instance-name validator. Applied to both `name` (when set)
/// and to the default (which is `distro`) so a reserved-slug base
/// distro can never be silently promoted to the instance name.
///
/// Rules: non-empty, len <= 64, `[A-Za-z0-9._-]` only, no shell
/// metacharacters, refuses Docker Desktop's reserved names.
pub fn validate_distro_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("[windows.wsl] name is empty".to_string());
    }
    if name.len() > 64 {
        return Err(format!(
            "[windows.wsl] name is longer than 64 characters ({} bytes)",
            name.len()
        ));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
    {
        return Err(format!(
            "[windows.wsl] name = \"{name}\" contains disallowed characters (only ASCII letters, digits, `.`, `_`, `-` are allowed)"
        ));
    }
    if RESERVED_DISTRO_NAMES
        .iter()
        .any(|r| r.eq_ignore_ascii_case(name))
    {
        return Err(format!(
            "[windows.wsl] name = \"{name}\" is reserved by Docker Desktop; pick a different name (e.g. \"jarvy-ubuntu\")"
        ));
    }
    Ok(())
}

/// Install-location validator. Accepts `"auto"` verbatim; anything
/// else must be an absolute Windows path with no UNC prefix, no
/// NUL / quotes / newlines. Path existence is NOT checked here —
/// bootstrap creates the directory.
pub fn validate_install_location(loc: &str) -> Result<(), String> {
    if loc == "auto" {
        return Ok(());
    }
    if loc.is_empty() {
        return Err("[windows.wsl] install_location is empty".to_string());
    }
    if loc.starts_with("\\\\") || loc.starts_with("//") {
        return Err(format!(
            "[windows.wsl] install_location = \"{loc}\" is a UNC path; WSL cannot import to a network location"
        ));
    }
    if loc.contains('\0') {
        return Err("[windows.wsl] install_location contains a NUL byte".to_string());
    }
    if loc.contains('"') {
        return Err("[windows.wsl] install_location contains a double-quote".to_string());
    }
    if loc.contains('\n') || loc.contains('\r') {
        return Err("[windows.wsl] install_location contains a newline".to_string());
    }
    if !is_absolute_windows_path(loc) {
        return Err(format!(
            "[windows.wsl] install_location = \"{loc}\" is not an absolute Windows path (expected e.g. `C:\\wsl\\jarvy`)"
        ));
    }
    Ok(())
}

fn is_absolute_windows_path(loc: &str) -> bool {
    // Rejects relative + drive-relative paths (`foo`, `\foo`, `C:foo`)
    // while accepting `C:\foo`, `D:/bar`, and the `%LOCALAPPDATA%\...`
    // form (env vars are expanded by the executor, not here).
    if loc.starts_with('%') {
        return true;
    }
    let bytes = loc.as_bytes();
    if bytes.len() < 3 {
        return false;
    }
    if !bytes[0].is_ascii_alphabetic() {
        return false;
    }
    if bytes[1] != b':' {
        return false;
    }
    matches!(bytes[2], b'\\' | b'/')
}

pub fn validate_channel(channel: &str) -> Result<(), String> {
    if BOUNDED_JARVY_CHANNELS
        .iter()
        .any(|c| c.eq_ignore_ascii_case(channel))
    {
        Ok(())
    } else {
        Err(format!(
            "[windows.wsl] jarvy_channel = \"{channel}\" is not bounded. Supported: {}",
            BOUNDED_JARVY_CHANNELS.join(", ")
        ))
    }
}

/// Errors returned by [`translate_win_path`].
#[derive(Debug, PartialEq, Eq)]
pub enum PathTranslateError {
    Unc,
    NotAbsolute,
    InvalidChars,
}

/// Translate a Windows path to its `/mnt/<drive>/...` WSL equivalent.
/// Pure logic, cross-platform testable. UNC paths and paths with
/// NUL / quotes / newlines are refused.
pub fn translate_win_path(path: &Path) -> Result<String, PathTranslateError> {
    let s = path.to_str().ok_or(PathTranslateError::InvalidChars)?;
    if s.starts_with("\\\\") || s.starts_with("//") {
        return Err(PathTranslateError::Unc);
    }
    if s.contains('\0') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        return Err(PathTranslateError::InvalidChars);
    }
    let bytes = s.as_bytes();
    if bytes.len() < 3 || !bytes[0].is_ascii_alphabetic() || bytes[1] != b':' {
        return Err(PathTranslateError::NotAbsolute);
    }
    if !matches!(bytes[2], b'\\' | b'/') {
        return Err(PathTranslateError::NotAbsolute);
    }
    let drive = (bytes[0] as char).to_ascii_lowercase();
    let rest: String = s[2..]
        .chars()
        .map(|c| if c == '\\' { '/' } else { c })
        .collect();
    Ok(format!("/mnt/{drive}{rest}"))
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

    #[test]
    fn wsl_absent_by_default() {
        let cfg: WindowsConfig = toml::from_str("").unwrap();
        assert!(cfg.wsl.is_none());
    }

    #[test]
    fn wsl_defaults_are_opt_in() {
        let cfg = WslConfig::default();
        assert!(!cfg.enabled);
        assert!(!cfg.auto_bootstrap);
        assert!(!cfg.run_setup);
        assert!(cfg.install_jarvy, "install_jarvy defaults on when enabled");
        assert_eq!(cfg.distro, "Ubuntu");
        assert_eq!(cfg.jarvy_channel, "stable");
        assert_eq!(cfg.install_location, "auto");
        assert!(cfg.name.is_none());
        assert_eq!(cfg.browser_launcher, BrowserLauncherMode::Off);
    }

    #[test]
    fn browser_launcher_defaults_to_off_on_empty_parse() {
        let cfg: WindowsConfig = toml::from_str("[wsl]").unwrap();
        let wsl = cfg.wsl.expect("wsl block present");
        assert_eq!(wsl.browser_launcher, BrowserLauncherMode::Off);
    }

    #[test]
    fn browser_launcher_parses_wslu_and_cmd_shim() {
        let cfg: WindowsConfig = toml::from_str(
            r#"
[wsl]
browser_launcher = "wslu"
"#,
        )
        .unwrap();
        assert_eq!(cfg.wsl.unwrap().browser_launcher, BrowserLauncherMode::Wslu);

        let cfg: WindowsConfig = toml::from_str(
            r#"
[wsl]
browser_launcher = "cmd_shim"
"#,
        )
        .unwrap();
        assert_eq!(
            cfg.wsl.unwrap().browser_launcher,
            BrowserLauncherMode::CmdShim
        );
    }

    #[test]
    fn validate_refuses_wslu_on_debian() {
        let cfg = WslConfig {
            distro: "Debian".to_string(),
            browser_launcher: BrowserLauncherMode::Wslu,
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn validate_allows_wslu_on_ubuntu() {
        let cfg = WslConfig {
            distro: "Ubuntu".to_string(),
            browser_launcher: BrowserLauncherMode::Wslu,
            ..Default::default()
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn validate_allows_cmd_shim_on_debian() {
        let cfg = WslConfig {
            distro: "Debian".to_string(),
            browser_launcher: BrowserLauncherMode::CmdShim,
            ..Default::default()
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn wsl_parses_full_block() {
        let toml_str = r#"
[wsl]
enabled = true
distro = "Debian"
name = "jarvy-debian"
auto_bootstrap = true
install_location = "C:\\wsl\\jarvy"
install_jarvy = true
jarvy_channel = "beta"
run_setup = true
browser_launcher = "cmd_shim"
"#;
        let cfg: WindowsConfig = toml::from_str(toml_str).unwrap();
        let wsl = cfg.wsl.expect("wsl block present");
        assert!(wsl.enabled);
        assert_eq!(wsl.distro, "Debian");
        assert_eq!(wsl.name.as_deref(), Some("jarvy-debian"));
        assert!(wsl.auto_bootstrap);
        assert_eq!(wsl.install_location, "C:\\wsl\\jarvy");
        assert!(wsl.install_jarvy);
        assert_eq!(wsl.jarvy_channel, "beta");
        assert!(wsl.run_setup);
        assert_eq!(wsl.browser_launcher, BrowserLauncherMode::CmdShim);
        assert!(wsl.validate().is_ok());
    }

    #[test]
    fn effective_name_defaults_to_distro() {
        let cfg = WslConfig {
            distro: "Ubuntu".to_string(),
            name: None,
            ..Default::default()
        };
        assert_eq!(cfg.effective_name(), "Ubuntu");
    }

    #[test]
    fn effective_name_uses_override() {
        let cfg = WslConfig {
            distro: "Ubuntu".to_string(),
            name: Some("jarvy-ubuntu".to_string()),
            ..Default::default()
        };
        assert_eq!(cfg.effective_name(), "jarvy-ubuntu");
    }

    #[test]
    fn distro_slug_accepts_supported() {
        assert!(validate_distro_slug("Ubuntu").is_ok());
        assert!(validate_distro_slug("Debian").is_ok());
        assert!(validate_distro_slug("ubuntu").is_ok(), "case-insensitive");
    }

    #[test]
    fn distro_slug_rejects_unsupported() {
        assert!(validate_distro_slug("Fedora").is_err());
        assert!(validate_distro_slug("Kali").is_err());
        assert!(validate_distro_slug("Alpine").is_err());
    }

    #[test]
    fn distro_name_accepts_typical() {
        assert!(validate_distro_name("Ubuntu").is_ok());
        assert!(validate_distro_name("jarvy-ubuntu").is_ok());
        assert!(validate_distro_name("Jarvy_2").is_ok());
        assert!(validate_distro_name("proj.dev").is_ok());
    }

    #[test]
    fn distro_name_rejects_shell_metachars() {
        assert!(validate_distro_name("foo; rm").is_err());
        assert!(validate_distro_name("foo --arg").is_err());
        assert!(validate_distro_name("foo\0evil").is_err());
        assert!(validate_distro_name("foo\"bar").is_err());
        assert!(validate_distro_name("foo bar").is_err());
        assert!(validate_distro_name("foo|bar").is_err());
    }

    #[test]
    fn distro_name_rejects_empty_and_oversize() {
        assert!(validate_distro_name("").is_err());
        let long = "a".repeat(65);
        assert!(validate_distro_name(&long).is_err());
    }

    #[test]
    fn distro_name_refuses_docker_desktop() {
        assert!(validate_distro_name("docker-desktop").is_err());
        assert!(validate_distro_name("docker-desktop-data").is_err());
        assert!(
            validate_distro_name("Docker-Desktop").is_err(),
            "case-insensitive"
        );
    }

    #[test]
    fn install_location_accepts_absolute_and_auto() {
        assert!(validate_install_location("auto").is_ok());
        assert!(validate_install_location("C:\\wsl\\jarvy").is_ok());
        assert!(validate_install_location("D:/tools/wsl").is_ok());
        assert!(
            validate_install_location("%LOCALAPPDATA%\\jarvy\\wsl").is_ok(),
            "env-var-prefixed paths are expanded by the executor"
        );
    }

    #[test]
    fn install_location_rejects_unc() {
        assert!(validate_install_location("\\\\server\\share").is_err());
        assert!(validate_install_location("//server/share").is_err());
    }

    #[test]
    fn install_location_rejects_relative() {
        assert!(validate_install_location("wsl").is_err());
        assert!(validate_install_location("./wsl").is_err());
        assert!(validate_install_location("\\foo").is_err());
    }

    #[test]
    fn install_location_rejects_bad_chars() {
        assert!(validate_install_location("C:\\wsl\0evil").is_err());
        assert!(validate_install_location("C:\\wsl\"bad").is_err());
        assert!(validate_install_location("C:\\wsl\nrm -rf").is_err());
    }

    #[test]
    fn channel_bounded() {
        assert!(validate_channel("stable").is_ok());
        assert!(validate_channel("beta").is_ok());
        assert!(validate_channel("nightly").is_ok());
        assert!(validate_channel("STABLE").is_ok(), "case-insensitive");
        assert!(validate_channel("edge").is_err());
        assert!(validate_channel("").is_err());
    }

    #[test]
    fn wsl_validate_refuses_unsupported_distro() {
        let cfg = WslConfig {
            distro: "Fedora".to_string(),
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn wsl_validate_refuses_reserved_name() {
        let cfg = WslConfig {
            name: Some("docker-desktop".to_string()),
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn path_translate_c_drive() {
        let out = translate_win_path(Path::new(r"C:\Users\me\proj")).unwrap();
        assert_eq!(out, "/mnt/c/Users/me/proj");
    }

    #[test]
    fn path_translate_lowercases_drive() {
        let out = translate_win_path(Path::new(r"D:\Projects\jarvy")).unwrap();
        assert_eq!(out, "/mnt/d/Projects/jarvy");
    }

    #[test]
    fn path_translate_forward_slash_input() {
        let out = translate_win_path(Path::new("E:/tools/foo")).unwrap();
        assert_eq!(out, "/mnt/e/tools/foo");
    }

    #[test]
    fn path_translate_refuses_unc() {
        let err = translate_win_path(Path::new(r"\\server\share\proj")).unwrap_err();
        assert_eq!(err, PathTranslateError::Unc);
    }

    #[test]
    fn path_translate_refuses_relative() {
        let err = translate_win_path(Path::new(r"proj\foo")).unwrap_err();
        assert_eq!(err, PathTranslateError::NotAbsolute);
    }

    #[test]
    fn path_translate_refuses_bad_chars() {
        let err = translate_win_path(Path::new("C:\\proj\0evil")).unwrap_err();
        assert_eq!(err, PathTranslateError::InvalidChars);
    }
}
