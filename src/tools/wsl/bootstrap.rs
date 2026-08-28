//! WSL2 probe, bootstrap, in-distro jarvy install, and setup delegation.
//!
//! Shell-outs (`wsl.exe`, `net.exe`, elevated PowerShell) are gated
//! behind `#[cfg(target_os = "windows")]`. Pure logic (argv builders,
//! probe parsing, plan selection, elevation-check parsing) is
//! cross-platform testable and lives outside the cfg gates.
//!
//! Jarvy never triggers UAC. When elevation is required, [`bootstrap`]
//! returns `RefusalReason::NotElevated` and the caller prints the
//! elevated command for the user to run manually.

// Pure-logic APIs (argv builders, decision seam, refusal renderers) are
// consumed by the Windows exec module and by cross-platform integration
// tests. On non-Windows builds without the integration test compiled in,
// rustc would flag them as dead. The tests cover every function.
#![allow(dead_code)]

use std::path::Path;
use std::sync::RwLock;

#[cfg(target_os = "windows")]
use crate::tools::common::InstallError;
use crate::windows::config::{PathTranslateError, WslConfig, translate_win_path};

use super::refusal::RefusalReason;

/// Process-wide effective [`WslConfig`], populated by the outer setup
/// dispatcher before tool installation runs. `None` means the setup
/// path never populated it (e.g. running `jarvy tools install wsl`
/// standalone before setup); the tool's `custom_install` then falls
/// back to a `WslConfig::default()` which has `enabled = false` and
/// so refuses cleanly with `RefusalReason::NotOptedIn`.
static ACTIVE_CONFIG: RwLock<Option<WslConfig>> = RwLock::new(None);

/// Snapshot the effective `[windows.wsl]` config for the current
/// invocation. Called from the setup dispatcher after the config load
/// completes.
pub fn set_active_config(cfg: WslConfig) {
    if let Ok(mut guard) = ACTIVE_CONFIG.write() {
        *guard = Some(cfg);
    }
}

/// Read a copy of the active `[windows.wsl]` config, or `None` when
/// no setup invocation populated it.
pub fn active_config() -> Option<WslConfig> {
    ACTIVE_CONFIG.read().ok().and_then(|g| g.clone())
}

/// Canonical URL for the jarvy install script piped into the WSL
/// distro when `install_jarvy = true`. Mirrors the URL end-user
/// installers use.
pub const JARVY_INSTALL_URL: &str =
    "https://raw.githubusercontent.com/kyou-tools/jarvy/main/dist/scripts/install.sh";

/// Snapshot of what `wsl --version` and `wsl -l -q` reported. Empty
/// `distros` with `store_based = false` means Store-WSL is not present
/// at all; empty with `store_based = true` means Store-WSL is present
/// but no distros are registered.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Probe {
    pub wsl_command_present: bool,
    pub store_based: bool,
    pub distros: Vec<String>,
    pub target_present: bool,
    pub jarvy_in_distro: bool,
}

/// What [`bootstrap`] did. `NoOp` means the target distro was already
/// present; `Installed` means jarvy ran `wsl --install` (or the
/// `--import` fallback) to create it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootstrapOutcome {
    NoOp,
    Installed { method: InstallMethod },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallMethod {
    WslInstall,
    WslImport,
}

impl InstallMethod {
    pub fn as_label(self) -> &'static str {
        match self {
            InstallMethod::WslInstall => "wsl_install",
            InstallMethod::WslImport => "wsl_import",
        }
    }
}

/// Errors returned by [`install_jarvy_inside`].
#[derive(Debug, thiserror::Error)]
pub enum InnerInstallError {
    #[error("wsl.exe not present")]
    NoWsl,
    #[error("curl-pipe of jarvy install exited {code:?}: {stderr}")]
    CommandFailed { code: Option<i32>, stderr: String },
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl InnerInstallError {
    pub fn kind(&self) -> &'static str {
        match self {
            InnerInstallError::NoWsl => "spawn",
            InnerInstallError::CommandFailed { .. } => "exit_nonzero",
            InnerInstallError::Io(_) => "network",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InnerInstallOutcome {
    AlreadyInstalled,
    Installed,
}

/// Errors returned by [`delegate_setup`].
#[derive(Debug, thiserror::Error)]
pub enum DelegateError {
    #[error("cwd path could not be translated to WSL: {0:?}")]
    Path(PathTranslateError),
    #[error("wsl.exe returned {code:?}: {stderr}")]
    Exit { code: Option<i32>, stderr: String },
    #[error("wsl.exe not present")]
    NoWsl,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl DelegateError {
    pub fn path_reason_label(&self) -> Option<&'static str> {
        match self {
            DelegateError::Path(PathTranslateError::Unc) => Some("unc"),
            DelegateError::Path(PathTranslateError::InvalidChars)
            | DelegateError::Path(PathTranslateError::NotAbsolute) => Some("invalid_chars"),
            _ => None,
        }
    }
}

//
// -- pure argv builders (cross-platform testable) ---------------------
//

/// Build the argv for `wsl --install -d <distro> --name <name>
/// --no-launch --location <install_location>`. Prefer this path;
/// fall back to [`argv_wsl_import`] when the caller's Store-WSL is
/// too old to accept `--name`.
pub fn argv_wsl_install(distro: &str, name: &str, install_location: &Path) -> Vec<String> {
    vec![
        "--install".to_string(),
        "-d".to_string(),
        distro.to_string(),
        "--name".to_string(),
        name.to_string(),
        "--no-launch".to_string(),
        "--location".to_string(),
        install_location.to_string_lossy().into_owned(),
    ]
}

/// Build the argv for `wsl --import <name> <install_location> <rootfs>`.
/// Used when `--name` is unsupported.
pub fn argv_wsl_import(name: &str, install_location: &Path, rootfs: &Path) -> Vec<String> {
    vec![
        "--import".to_string(),
        name.to_string(),
        install_location.to_string_lossy().into_owned(),
        rootfs.to_string_lossy().into_owned(),
    ]
}

/// Build the argv for `wsl -d <name> -u root -- bash -lc "curl ... | sh ..."`.
/// The URL is a compile-time constant; the channel comes from
/// [`WslConfig`] and is bounded by `validate_channel` at config-load.
pub fn argv_wsl_jarvy_install(name: &str, channel: &str) -> Vec<String> {
    let cmd = format!(
        "curl -fsSL {url} | sh -s -- --channel {channel}",
        url = JARVY_INSTALL_URL,
        channel = channel,
    );
    vec![
        "-d".to_string(),
        name.to_string(),
        "-u".to_string(),
        "root".to_string(),
        "--".to_string(),
        "bash".to_string(),
        "-lc".to_string(),
        cmd,
    ]
}

/// Build the argv for `wsl -d <name> -- bash -lc "cd '<wsl_cwd>' &&
/// jarvy setup [--from <url>]"`. `wsl_cwd` MUST be pre-translated via
/// [`translate_win_path`] — this function assumes a valid POSIX path.
pub fn argv_wsl_delegate_setup(name: &str, wsl_cwd: &str, from_url: Option<&str>) -> Vec<String> {
    let cwd_quoted = shell_single_quote(wsl_cwd);
    let mut inner = format!("cd {cwd_quoted} && jarvy setup");
    if let Some(url) = from_url {
        inner.push_str(" --from ");
        inner.push_str(&shell_single_quote(url));
    }
    vec![
        "-d".to_string(),
        name.to_string(),
        "--".to_string(),
        "bash".to_string(),
        "-lc".to_string(),
        inner,
    ]
}

/// Wrap a string in POSIX single quotes for safe use inside a
/// `bash -lc "..."` payload. Escapes embedded single quotes via the
/// standard `'\''` idiom.
pub fn shell_single_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        if c == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

/// Parse `wsl --version` output. Store-WSL prints a table starting
/// with `WSL version:` or `WSL 版本:`; legacy in-box WSL prints
/// nothing (or an error to stderr, which we ignore). Any non-empty
/// stdout starting with `WSL` counts.
pub fn parse_wsl_version_store_based(stdout: &str) -> bool {
    let trimmed = stdout.trim_start_matches('\u{feff}').trim();
    !trimmed.is_empty()
        && trimmed
            .lines()
            .next()
            .map(|l| l.trim_start().starts_with("WSL"))
            .unwrap_or(false)
}

/// Parse `wsl -l -q` output — one distro name per line. UTF-16LE
/// output on some Windows versions is normalized to UTF-8 by the
/// caller before this runs (the caller strips a BOM if present).
pub fn parse_wsl_list_quiet(stdout: &str) -> Vec<String> {
    stdout
        .trim_start_matches('\u{feff}')
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

//
// -- decision seam (cross-platform testable) --------------------------
//

/// Snapshot of runtime knobs the bootstrap decision depends on.
/// Broken out so `wsl_should_bootstrap` stays pure — tests pass a
/// fabricated context instead of touching the real subsystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BootstrapContext {
    pub wsl_present: bool,
    pub store_based: bool,
    pub target_present: bool,
    pub elevated: bool,
}

/// What bootstrap should do given a config + runtime probe. Splits
/// the "refuse cleanly" cases from the "run the install" case so the
/// caller can attach different telemetry / stderr shapes without
/// re-deriving the decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootstrapDecision {
    /// Distro already registered; nothing to do.
    NoOp,
    /// Run `wsl --install -d <distro> --name <name>`.
    RunInstall,
    /// `--install --name` is expected to be unavailable (very old
    /// Store-WSL); use `wsl --import` with a cached rootfs. v1 always
    /// tries `--install` first and only falls back on runtime error;
    /// this variant exists for future planners that want to short-cut
    /// based on a version probe.
    RunImport,
    /// Refuse with a specific reason.
    Refuse(RefusalReason),
}

/// Pure decision — no side effects, no shell-outs.
pub fn wsl_should_bootstrap(cfg: &WslConfig, ctx: BootstrapContext) -> BootstrapDecision {
    if !cfg.enabled {
        return BootstrapDecision::Refuse(RefusalReason::NotOptedIn);
    }
    if !ctx.wsl_present {
        return BootstrapDecision::Refuse(RefusalReason::NoWslCommand);
    }
    if !ctx.store_based {
        return BootstrapDecision::Refuse(RefusalReason::NotStoreWsl);
    }
    if ctx.target_present {
        return BootstrapDecision::NoOp;
    }
    if !cfg.auto_bootstrap {
        return BootstrapDecision::Refuse(RefusalReason::AutoBootstrapOff);
    }
    if !ctx.elevated {
        return BootstrapDecision::Refuse(RefusalReason::NotElevated);
    }
    BootstrapDecision::RunInstall
}

//
// -- refusal-message renderer (cross-platform testable) ---------------
//

/// Human-facing refusal message for a given reason + config. The
/// output is printed to stderr by callers; every reason includes a
/// copy-pasteable next-step command so the user knows exactly what
/// to do next.
pub fn render_refusal(cfg: &WslConfig, reason: RefusalReason) -> String {
    let name = cfg.effective_name();
    let distro = &cfg.distro;
    let location = cfg.effective_install_location();
    let location_display = location.display();
    match reason {
        RefusalReason::NotOptedIn => {
            "[wsl] Not enabled. Set `[windows.wsl] enabled = true` in jarvy.toml to opt in."
                .to_string()
        }
        RefusalReason::NoWslCommand => "[wsl] `wsl.exe` is not on PATH. Install WSL2 once in an elevated PowerShell:\n    wsl --install --no-launch\nThen reboot and re-run jarvy setup.".to_string(),
        RefusalReason::NotStoreWsl => "[wsl] WSL2 not detected as Store-based. Jarvy refuses to touch DISM.\nRun once in an elevated PowerShell, then reboot:\n    wsl --install --no-launch\nThen re-run jarvy setup.".to_string(),
        RefusalReason::AutoBootstrapOff => format!(
            "[wsl] Distro \"{name}\" not installed and auto_bootstrap = false.\nRun once in an elevated PowerShell:\n    wsl --install -d {distro} --name {name} --no-launch --location \"{location_display}\"\nThen launch the distro once to create your user, and re-run jarvy setup."
        ),
        RefusalReason::NotElevated => format!(
            "[wsl] Distro \"{name}\" not installed and jarvy cannot elevate.\nRun once in an elevated PowerShell:\n    wsl --install -d {distro} --name {name} --no-launch --location \"{location_display}\"\nThen re-run jarvy setup."
        ),
        RefusalReason::ReservedName => format!(
            "[wsl] name = \"{name}\" is reserved by Docker Desktop. Pick a different name (e.g. \"jarvy-ubuntu\")."
        ),
    }
}

/// Message printed when `jarvy tools remove wsl` is called. Destructive
/// action policy: never silent, never automatic.
pub fn render_remove_refusal(cfg: &WslConfig) -> String {
    let name = cfg.effective_name();
    format!(
        "[wsl] Removing WSL is destructive; jarvy will not `wsl --unregister` your distro.\nRun manually if you're sure (this wipes the distro's rootfs including any user data):\n    wsl --unregister {name}"
    )
}

//
// -- Windows-only executors ------------------------------------------
//

#[cfg(target_os = "windows")]
mod exec {
    use super::*;
    use crate::tools::common::{has, run};

    pub fn probe(cfg: &WslConfig) -> Probe {
        if !has("wsl") {
            return Probe::default();
        }
        let mut probe = Probe {
            wsl_command_present: true,
            ..Default::default()
        };
        // wsl --version — Store WSL only
        if let Ok(out) = run("wsl", &["--version"]) {
            let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
            probe.store_based = parse_wsl_version_store_based(&stdout);
        }
        // wsl -l -q — list registered distros
        if let Ok(out) = run("wsl", &["-l", "-q"]) {
            // wsl -l may emit UTF-16LE on some Windows versions; normalize
            let bytes = &out.stdout;
            let text = if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
                // UTF-16LE BOM — decode
                let u16s: Vec<u16> = bytes[2..]
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|c| u16::from_le_bytes(*c))
                    .collect();
                String::from_utf16_lossy(&u16s)
            } else {
                String::from_utf8_lossy(bytes).into_owned()
            };
            probe.distros = parse_wsl_list_quiet(&text);
        }
        let name = cfg.effective_name();
        probe.target_present = probe.distros.iter().any(|d| d.eq_ignore_ascii_case(name));
        if probe.target_present {
            probe.jarvy_in_distro = jarvy_in_distro(name);
        }
        emit_probe(&probe);
        probe
    }

    pub fn is_elevated() -> bool {
        // `net.exe session` succeeds only when the current process is
        // elevated; non-elevated returns exit 2 with "Access is denied".
        // 20ms; standard idiom on Windows.
        run("net", &["session"]).is_ok()
    }

    pub fn jarvy_in_distro(name: &str) -> bool {
        run(
            "wsl",
            &[
                "-d",
                name,
                "--",
                "sh",
                "-lc",
                "command -v jarvy >/dev/null 2>&1",
            ],
        )
        .is_ok()
    }

    pub fn bootstrap(cfg: &WslConfig) -> Result<BootstrapOutcome, RefusalReason> {
        let probe = probe(cfg);
        let ctx = BootstrapContext {
            wsl_present: probe.wsl_command_present,
            store_based: probe.store_based,
            target_present: probe.target_present,
            elevated: is_elevated(),
        };
        match wsl_should_bootstrap(cfg, ctx) {
            BootstrapDecision::NoOp => Ok(BootstrapOutcome::NoOp),
            BootstrapDecision::RunInstall => run_wsl_install(cfg),
            BootstrapDecision::RunImport => run_wsl_import(cfg),
            BootstrapDecision::Refuse(r) => {
                emit_refused(r);
                Err(r)
            }
        }
    }

    fn run_wsl_install(cfg: &WslConfig) -> Result<BootstrapOutcome, RefusalReason> {
        let location = cfg.effective_install_location();
        if let Err(e) = std::fs::create_dir_all(&location) {
            emit_bootstrap_failed(
                &cfg.distro,
                InstallMethod::WslInstall,
                "location_invalid",
                &e.to_string(),
            );
            // Fall through to `--import` may still work if the dir issue
            // was a spurious ACL glitch, but usually this is fatal.
            return Err(RefusalReason::AutoBootstrapOff);
        }
        emit_bootstrap_started(&cfg.distro);
        let started = std::time::Instant::now();
        let argv = argv_wsl_install(&cfg.distro, cfg.effective_name(), &location);
        let arg_refs: Vec<&str> = argv.iter().map(String::as_str).collect();
        match run("wsl", &arg_refs) {
            Ok(_) => {
                emit_bootstrap_completed(&cfg.distro, InstallMethod::WslInstall, started.elapsed());
                Ok(BootstrapOutcome::Installed {
                    method: InstallMethod::WslInstall,
                })
            }
            Err(e) => {
                let stderr = install_error_stderr(&e);
                let looks_like_flag_unsupported =
                    stderr.to_ascii_lowercase().contains("unrecognized")
                        || stderr.to_ascii_lowercase().contains("--name");
                if looks_like_flag_unsupported {
                    // Fall back to `wsl --import`
                    tracing::info!(
                        event = "wsl.bootstrap_install_flag_unsupported",
                        "falling back to wsl --import"
                    );
                    run_wsl_import(cfg)
                } else {
                    emit_bootstrap_failed(
                        &cfg.distro,
                        InstallMethod::WslInstall,
                        install_error_cause(&e),
                        &stderr,
                    );
                    Err(RefusalReason::AutoBootstrapOff)
                }
            }
        }
    }

    fn run_wsl_import(cfg: &WslConfig) -> Result<BootstrapOutcome, RefusalReason> {
        let location = cfg.effective_install_location();
        let Some(entry) = crate::tools::wsl::rootfs::lookup(&cfg.distro) else {
            emit_bootstrap_failed(
                &cfg.distro,
                InstallMethod::WslImport,
                "rootfs_download",
                "no pinned rootfs for distro",
            );
            return Err(RefusalReason::AutoBootstrapOff);
        };
        let Some(rootfs_path) = crate::tools::wsl::rootfs::cache_path(entry) else {
            emit_bootstrap_failed(
                &cfg.distro,
                InstallMethod::WslImport,
                "rootfs_download",
                "cache path unavailable",
            );
            return Err(RefusalReason::AutoBootstrapOff);
        };
        if !crate::tools::wsl::rootfs::cache_hit(entry, &rootfs_path)
            && let Err(e) = fetch_rootfs(entry.url, &rootfs_path)
        {
            emit_bootstrap_failed(
                &cfg.distro,
                InstallMethod::WslImport,
                "rootfs_download",
                &e.to_string(),
            );
            return Err(RefusalReason::AutoBootstrapOff);
        }
        if let Err(e) = std::fs::create_dir_all(&location) {
            emit_bootstrap_failed(
                &cfg.distro,
                InstallMethod::WslImport,
                "location_invalid",
                &e.to_string(),
            );
            return Err(RefusalReason::AutoBootstrapOff);
        }
        emit_bootstrap_started(&cfg.distro);
        let started = std::time::Instant::now();
        let argv = argv_wsl_import(cfg.effective_name(), &location, &rootfs_path);
        let arg_refs: Vec<&str> = argv.iter().map(String::as_str).collect();
        match run("wsl", &arg_refs) {
            Ok(_) => {
                emit_bootstrap_completed(&cfg.distro, InstallMethod::WslImport, started.elapsed());
                Ok(BootstrapOutcome::Installed {
                    method: InstallMethod::WslImport,
                })
            }
            Err(e) => {
                let stderr = install_error_stderr(&e);
                emit_bootstrap_failed(
                    &cfg.distro,
                    InstallMethod::WslImport,
                    install_error_cause(&e),
                    &stderr,
                );
                Err(RefusalReason::AutoBootstrapOff)
            }
        }
    }

    fn fetch_rootfs(url: &str, dest: &Path) -> Result<(), std::io::Error> {
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        // Delegate to jarvy's shared bounded-fetch primitive.
        let bytes = crate::net::bounded_fetch::bounded_fetch(
            url,
            700 * 1024 * 1024, // 700 MB cap — WSL rootfs typically 300-400 MB
            crate::net::bounded_fetch::BoundedFetchConfig {
                insecure_loopback_env: "JARVY_LIBRARY_ALLOW_INSECURE_FETCH",
            },
        )
        .map_err(|e| std::io::Error::other(format!("rootfs fetch failed: {e:?}")))?;
        std::fs::write(dest, bytes)?;
        Ok(())
    }

    pub fn install_jarvy_inside(cfg: &WslConfig) -> Result<InnerInstallOutcome, InnerInstallError> {
        if !has("wsl") {
            return Err(InnerInstallError::NoWsl);
        }
        let name = cfg.effective_name();
        if jarvy_in_distro(name) {
            return Ok(InnerInstallOutcome::AlreadyInstalled);
        }
        emit_jarvy_install_started(&cfg.distro, &cfg.jarvy_channel);
        let started = std::time::Instant::now();
        let argv = argv_wsl_jarvy_install(name, &cfg.jarvy_channel);
        let arg_refs: Vec<&str> = argv.iter().map(String::as_str).collect();
        match run("wsl", &arg_refs) {
            Ok(_) => {
                emit_jarvy_install_completed(&cfg.distro, &cfg.jarvy_channel, started.elapsed());
                Ok(InnerInstallOutcome::Installed)
            }
            Err(e) => {
                let stderr = install_error_stderr(&e);
                let code = install_error_code(&e);
                emit_jarvy_install_failed(
                    &cfg.distro,
                    &cfg.jarvy_channel,
                    install_error_cause(&e),
                    &stderr,
                );
                Err(InnerInstallError::CommandFailed { code, stderr })
            }
        }
    }

    pub fn delegate_setup(
        cfg: &WslConfig,
        cwd: &Path,
        from_url: Option<&str>,
    ) -> Result<i32, DelegateError> {
        if !has("wsl") {
            return Err(DelegateError::NoWsl);
        }
        let wsl_cwd = translate_win_path(cwd).map_err(DelegateError::Path)?;
        emit_delegated_started(&cfg.distro);
        let started = std::time::Instant::now();
        let argv = argv_wsl_delegate_setup(cfg.effective_name(), &wsl_cwd, from_url);
        let arg_refs: Vec<&str> = argv.iter().map(String::as_str).collect();
        let mut cmd = std::process::Command::new("wsl");
        cmd.args(&arg_refs);
        cmd.stdin(std::process::Stdio::inherit());
        cmd.stdout(std::process::Stdio::inherit());
        cmd.stderr(std::process::Stdio::inherit());
        let status = cmd.status()?;
        let exit_code = status.code().unwrap_or(-1);
        emit_delegated_completed(&cfg.distro, exit_code, started.elapsed());
        Ok(exit_code)
    }

    fn install_error_stderr(e: &InstallError) -> String {
        match e {
            InstallError::CommandFailed { stderr, .. } => stderr.clone(),
            other => other.to_string(),
        }
    }

    fn install_error_code(e: &InstallError) -> Option<i32> {
        match e {
            InstallError::CommandFailed { code, .. } => *code,
            _ => None,
        }
    }

    fn install_error_cause(e: &InstallError) -> &'static str {
        match e {
            InstallError::CommandFailed { .. } => "exit_nonzero",
            InstallError::Io(_) => "spawn",
            InstallError::Prereq(_) => "spawn",
            InstallError::InvalidPermissions(_) => "spawn",
            InstallError::Parse(_) => "spawn",
            InstallError::Unsupported => "spawn",
        }
    }
}

#[cfg(target_os = "windows")]
pub use exec::{bootstrap, delegate_setup, install_jarvy_inside};

/// Cross-platform stubs so callers can compile everywhere. The
/// non-Windows path always returns `NoOp` / `false` / a `NoWsl`-shaped
/// error — the tool's registration surfaces `Unsupported` first.
#[cfg(not(target_os = "windows"))]
pub fn probe(_cfg: &WslConfig) -> Probe {
    Probe::default()
}

#[cfg(not(target_os = "windows"))]
pub fn is_elevated() -> bool {
    false
}

#[cfg(not(target_os = "windows"))]
pub fn bootstrap(_cfg: &WslConfig) -> Result<BootstrapOutcome, RefusalReason> {
    Err(RefusalReason::NoWslCommand)
}

#[cfg(not(target_os = "windows"))]
pub fn install_jarvy_inside(_cfg: &WslConfig) -> Result<InnerInstallOutcome, InnerInstallError> {
    Err(InnerInstallError::NoWsl)
}

#[cfg(not(target_os = "windows"))]
pub fn delegate_setup(
    _cfg: &WslConfig,
    _cwd: &Path,
    _from_url: Option<&str>,
) -> Result<i32, DelegateError> {
    Err(DelegateError::NoWsl)
}

/// Silence the unused-path lint on non-Windows builds where
/// [`translate_win_path`] is only used by the Windows executor.
#[cfg(not(target_os = "windows"))]
fn _keep_import_alive() {
    let _ = translate_win_path;
}

//
// -- telemetry --------------------------------------------------------
//

fn gate() -> bool {
    crate::observability::telemetry_gate::is_enabled()
}

#[allow(dead_code)]
fn emit_probe(probe: &Probe) {
    if !gate() {
        return;
    }
    tracing::info!(
        event = "wsl.probe_completed",
        store_based = probe.store_based,
        distros_installed = probe.distros.len() as u64,
        target_distro_present = probe.target_present,
        jarvy_in_distro = probe.jarvy_in_distro,
    );
}

#[allow(dead_code)]
fn emit_refused(reason: RefusalReason) {
    if !gate() {
        return;
    }
    tracing::warn!(event = "wsl.bootstrap_refused", reason = reason.as_label());
}

#[allow(dead_code)]
fn emit_bootstrap_started(base_distro: &str) {
    if !gate() {
        return;
    }
    tracing::info!(event = "wsl.bootstrap_started", base_distro = %base_distro);
}

#[allow(dead_code)]
fn emit_bootstrap_completed(
    base_distro: &str,
    method: InstallMethod,
    duration: std::time::Duration,
) {
    if !gate() {
        return;
    }
    tracing::info!(
        event = "wsl.bootstrap_completed",
        base_distro = %base_distro,
        method = method.as_label(),
        duration_ms = duration.as_millis() as u64,
    );
}

#[allow(dead_code)]
fn emit_bootstrap_failed(base_distro: &str, method: InstallMethod, cause: &str, error: &str) {
    if !gate() {
        return;
    }
    tracing::warn!(
        event = "wsl.bootstrap_failed",
        base_distro = %base_distro,
        method = method.as_label(),
        cause = %cause,
        error = %error,
    );
}

#[allow(dead_code)]
fn emit_jarvy_install_started(base_distro: &str, channel: &str) {
    if !gate() {
        return;
    }
    tracing::info!(
        event = "wsl.jarvy_install_started",
        base_distro = %base_distro,
        channel = %channel,
    );
}

#[allow(dead_code)]
fn emit_jarvy_install_completed(base_distro: &str, channel: &str, duration: std::time::Duration) {
    if !gate() {
        return;
    }
    tracing::info!(
        event = "wsl.jarvy_install_completed",
        base_distro = %base_distro,
        channel = %channel,
        duration_ms = duration.as_millis() as u64,
    );
}

#[allow(dead_code)]
fn emit_jarvy_install_failed(base_distro: &str, channel: &str, cause: &str, error: &str) {
    if !gate() {
        return;
    }
    tracing::warn!(
        event = "wsl.jarvy_install_failed",
        base_distro = %base_distro,
        channel = %channel,
        cause = %cause,
        error = %error,
    );
}

#[allow(dead_code)]
fn emit_delegated_started(base_distro: &str) {
    if !gate() {
        return;
    }
    tracing::debug!(event = "wsl.setup_delegated_started", base_distro = %base_distro);
}

#[allow(dead_code)]
fn emit_delegated_completed(base_distro: &str, exit_code: i32, duration: std::time::Duration) {
    if !gate() {
        return;
    }
    tracing::info!(
        event = "wsl.setup_delegated",
        base_distro = %base_distro,
        exit_code,
        duration_ms = duration.as_millis() as u64,
    );
}

/// Emit `wsl.path_refused` — used when delegation cwd cannot be
/// translated. Called by the delegation site in `setup_cmd`, not by
/// `delegate_setup` (the error is surfaced to the caller which owns
/// the printed refusal message).
pub fn emit_path_refused(reason: &'static str) {
    if !gate() {
        return;
    }
    tracing::warn!(event = "wsl.path_refused", reason);
}

/// Emit `wsl.remove_refused` — used by `jarvy tools remove wsl`.
pub fn emit_remove_refused() {
    if !gate() {
        return;
    }
    tracing::warn!(event = "wsl.remove_refused");
}

//
// -- tests ------------------------------------------------------------
//

#[cfg(test)]
mod tests {
    use super::*;

    fn base_cfg() -> WslConfig {
        WslConfig {
            enabled: true,
            distro: "Ubuntu".to_string(),
            name: None,
            auto_bootstrap: true,
            install_location: "C:\\wsl\\jarvy".to_string(),
            install_jarvy: true,
            jarvy_channel: "stable".to_string(),
            run_setup: false,
        }
    }

    fn base_ctx() -> BootstrapContext {
        BootstrapContext {
            wsl_present: true,
            store_based: true,
            target_present: false,
            elevated: true,
        }
    }

    #[test]
    fn decision_noop_when_opted_out() {
        let cfg = WslConfig::default(); // enabled = false
        assert_eq!(
            wsl_should_bootstrap(&cfg, base_ctx()),
            BootstrapDecision::Refuse(RefusalReason::NotOptedIn)
        );
    }

    #[test]
    fn decision_refuses_when_wsl_missing() {
        let ctx = BootstrapContext {
            wsl_present: false,
            ..base_ctx()
        };
        assert_eq!(
            wsl_should_bootstrap(&base_cfg(), ctx),
            BootstrapDecision::Refuse(RefusalReason::NoWslCommand)
        );
    }

    #[test]
    fn decision_refuses_when_not_store_wsl() {
        let ctx = BootstrapContext {
            store_based: false,
            ..base_ctx()
        };
        assert_eq!(
            wsl_should_bootstrap(&base_cfg(), ctx),
            BootstrapDecision::Refuse(RefusalReason::NotStoreWsl)
        );
    }

    #[test]
    fn decision_noop_when_target_present() {
        let ctx = BootstrapContext {
            target_present: true,
            ..base_ctx()
        };
        assert_eq!(
            wsl_should_bootstrap(&base_cfg(), ctx),
            BootstrapDecision::NoOp
        );
    }

    #[test]
    fn decision_refuses_when_auto_bootstrap_off() {
        let cfg = WslConfig {
            auto_bootstrap: false,
            ..base_cfg()
        };
        assert_eq!(
            wsl_should_bootstrap(&cfg, base_ctx()),
            BootstrapDecision::Refuse(RefusalReason::AutoBootstrapOff)
        );
    }

    #[test]
    fn decision_refuses_when_not_elevated() {
        let ctx = BootstrapContext {
            elevated: false,
            ..base_ctx()
        };
        assert_eq!(
            wsl_should_bootstrap(&base_cfg(), ctx),
            BootstrapDecision::Refuse(RefusalReason::NotElevated)
        );
    }

    #[test]
    fn decision_runs_install_when_all_gates_pass() {
        assert_eq!(
            wsl_should_bootstrap(&base_cfg(), base_ctx()),
            BootstrapDecision::RunInstall
        );
    }

    #[test]
    fn argv_install_shape() {
        let argv = argv_wsl_install("Ubuntu", "Ubuntu", Path::new("C:\\wsl\\jarvy"));
        assert_eq!(
            argv,
            vec![
                "--install",
                "-d",
                "Ubuntu",
                "--name",
                "Ubuntu",
                "--no-launch",
                "--location",
                "C:\\wsl\\jarvy",
            ]
        );
    }

    #[test]
    fn argv_import_shape() {
        let argv = argv_wsl_import(
            "jarvy-ubuntu",
            Path::new("C:\\wsl\\jarvy"),
            Path::new("C:\\cache\\rootfs.tar.gz"),
        );
        assert_eq!(
            argv,
            vec![
                "--import",
                "jarvy-ubuntu",
                "C:\\wsl\\jarvy",
                "C:\\cache\\rootfs.tar.gz",
            ]
        );
    }

    #[test]
    fn argv_jarvy_install_shape() {
        let argv = argv_wsl_jarvy_install("Ubuntu", "stable");
        assert_eq!(argv[0], "-d");
        assert_eq!(argv[1], "Ubuntu");
        assert_eq!(argv[2], "-u");
        assert_eq!(argv[3], "root");
        assert_eq!(argv[4], "--");
        assert_eq!(argv[5], "bash");
        assert_eq!(argv[6], "-lc");
        assert!(argv[7].contains("curl -fsSL"));
        assert!(argv[7].contains("--channel stable"));
    }

    #[test]
    fn argv_delegate_setup_shape() {
        let argv = argv_wsl_delegate_setup("Ubuntu", "/mnt/c/proj", None);
        assert_eq!(
            argv,
            vec![
                "-d".to_string(),
                "Ubuntu".to_string(),
                "--".to_string(),
                "bash".to_string(),
                "-lc".to_string(),
                "cd '/mnt/c/proj' && jarvy setup".to_string(),
            ]
        );
    }

    #[test]
    fn argv_delegate_setup_propagates_from_url() {
        let argv = argv_wsl_delegate_setup(
            "Ubuntu",
            "/mnt/c/proj",
            Some("https://example.com/config.toml"),
        );
        assert_eq!(
            argv[5],
            "cd '/mnt/c/proj' && jarvy setup --from 'https://example.com/config.toml'"
        );
    }

    #[test]
    fn shell_single_quote_escapes_apostrophes() {
        assert_eq!(shell_single_quote("plain"), "'plain'");
        assert_eq!(shell_single_quote("with'apo"), "'with'\\''apo'");
        assert_eq!(
            shell_single_quote("multi'quo'tes"),
            "'multi'\\''quo'\\''tes'"
        );
    }

    #[test]
    fn parse_version_recognizes_store_output() {
        assert!(parse_wsl_version_store_based(
            "WSL version: 2.0.14.0\nKernel version: 5.15.133.1-1"
        ));
        assert!(parse_wsl_version_store_based(" WSL 版本: 2.0.14.0"));
        assert!(!parse_wsl_version_store_based(""));
        assert!(!parse_wsl_version_store_based(
            "The Windows Subsystem for Linux optional component is not enabled"
        ));
    }

    #[test]
    fn parse_list_quiet_yields_names() {
        assert_eq!(
            parse_wsl_list_quiet("Ubuntu\nDebian\n"),
            vec!["Ubuntu".to_string(), "Debian".to_string()]
        );
        assert!(parse_wsl_list_quiet("").is_empty());
        assert_eq!(
            parse_wsl_list_quiet("\u{feff}Ubuntu\n"),
            vec!["Ubuntu".to_string()]
        );
    }

    #[test]
    fn refusal_labels_are_bounded() {
        for r in [
            RefusalReason::NotOptedIn,
            RefusalReason::NotStoreWsl,
            RefusalReason::NoWslCommand,
            RefusalReason::AutoBootstrapOff,
            RefusalReason::NotElevated,
            RefusalReason::ReservedName,
        ] {
            let label = r.as_label();
            assert!(!label.is_empty());
            assert!(label.chars().all(|c| c.is_ascii_lowercase() || c == '_'));
        }
    }

    #[test]
    fn refusal_message_includes_install_command_for_auto_bootstrap_off() {
        let cfg = base_cfg();
        let msg = render_refusal(&cfg, RefusalReason::AutoBootstrapOff);
        assert!(msg.contains("wsl --install"));
        assert!(msg.contains("--name Ubuntu"));
    }

    #[test]
    fn refusal_message_includes_install_command_for_not_elevated() {
        let cfg = base_cfg();
        let msg = render_refusal(&cfg, RefusalReason::NotElevated);
        assert!(msg.contains("wsl --install"));
    }

    #[test]
    fn refusal_message_for_not_store_wsl_avoids_dism() {
        let cfg = base_cfg();
        let msg = render_refusal(&cfg, RefusalReason::NotStoreWsl);
        assert!(msg.contains("DISM"));
        assert!(msg.contains("Store-based") || msg.to_lowercase().contains("store"));
    }

    #[test]
    fn remove_refusal_includes_unregister_hint() {
        let cfg = base_cfg();
        let msg = render_remove_refusal(&cfg);
        assert!(msg.contains("wsl --unregister"));
    }

    #[test]
    fn install_method_labels() {
        assert_eq!(InstallMethod::WslInstall.as_label(), "wsl_install");
        assert_eq!(InstallMethod::WslImport.as_label(), "wsl_import");
    }
}
