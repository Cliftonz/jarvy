//! Git hook management (PRD-048)
//!
//! Installs and manages Git hooks driven by `jarvy.toml`'s
//! `[git_hooks]` block. The **native** handler writes hook scripts
//! directly into `.git/hooks/<stage>` — no third-party framework in
//! the loop — with sources from inline TOML bodies, project-local
//! files, or scanned folders. Husky and Lefthook are stubbed for
//! auto-detection but not yet installed.
//!
//! # Why `[git_hooks]` and not `[hooks]`
//!
//! `[hooks]` is already used by `jarvy setup` for `pre_setup` /
//! `post_install` / `post_setup` shell scripts (PRD-003). Adding a
//! `git_hooks = true` knob into that existing block would entangle two
//! unrelated lifecycles. Using a new top-level `[git_hooks]` keeps
//! their semantics independent and lets users mix-and-match (no setup
//! hooks but yes git hooks, or vice versa).
//!
//! # Trust boundary
//!
//! Native hook scripts execute on `git commit` / `git push` / etc. —
//! the user must already trust their own repo. Remote configs fetched
//! via `jarvy setup --from <url>` are blocked from auto-installing
//! hooks unless `[git_hooks] allow_remote = true` is set in the
//! source config (mirrors `[packages] allow_remote`). File-reference
//! and folder-scan sources refuse absolute paths, `..` traversal, and
//! symlink escape via `resolve_project_path`.

pub mod config;
pub mod detection;
pub mod husky;
pub mod lefthook;
pub mod native;
pub mod repo;

use std::path::Path;
use thiserror::Error;

pub use config::{GitHooksConfig, HookFramework};
pub use detection::detect_framework;

/// Errors produced by hook installation / management.
#[derive(Debug, Error)]
pub enum HookError {
    #[error("hook framework `{0}` is not installed; install it before running `jarvy hooks`")]
    FrameworkNotInstalled(String),

    /// Reserved for future frameworks that get declared in
    /// `HookFramework` before a handler ships. With pre-commit /
    /// husky / lefthook / native all wired today this variant is
    /// dormant but kept so adding a new enum variant produces a
    /// clean "not yet supported" error instead of a panic.
    #[allow(dead_code)]
    #[error("hook framework `{0}` is configured but not yet supported by jarvy")]
    UnsupportedFramework(String),

    #[error(
        "remote-fetched config attempted to install git hooks without \
         `[git_hooks] allow_remote = true`; refusing to land arbitrary \
         pre-commit hooks from an untrusted source (PRD-054 trust gate)"
    )]
    RemoteRefused,

    #[error("not inside a git repository (no `.git` directory found)")]
    NotAGitRepo,

    #[error("hook installation failed: {0}")]
    InstallFailed(String),

    #[error("hook update failed: {0}")]
    UpdateFailed(String),

    #[error("hook run failed: {0}")]
    RunFailed(String),

    #[error("config error: {0}")]
    Config(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

impl HookError {
    /// Stable telemetry discriminant. Mirrors the `kind()` pattern used by
    /// `PackageError` and `AiHookError`.
    pub fn kind(&self) -> &'static str {
        match self {
            HookError::FrameworkNotInstalled(_) => "framework_not_installed",
            HookError::UnsupportedFramework(_) => "unsupported_framework",
            HookError::RemoteRefused => "remote_refused",
            HookError::NotAGitRepo => "not_a_git_repo",
            HookError::InstallFailed(_) => "install_failed",
            HookError::UpdateFailed(_) => "update_failed",
            HookError::RunFailed(_) => "run_failed",
            HookError::Config(_) => "config",
            HookError::Io(_) => "io",
        }
    }
}

/// Refuse install / update / run when the config came from a remote
/// `jarvy setup --from <url>` source and `allow_remote` is not set.
/// Review item 5 (P0) — previously the `allow_remote` field was
/// declared but never read, so a friendly-looking remote config could
/// land arbitrary pre-commit hooks on the consuming machine.
fn enforce_remote_gate(config: &GitHooksConfig) -> Result<(), HookError> {
    if config.origin == crate::ai_hooks::ConfigOrigin::Remote && !config.allow_remote {
        if crate::observability::telemetry_gate::is_enabled() {
            tracing::warn!(
                event = "git_hooks.remote_refused",
                reason = "allow_remote_not_set",
            );
        }
        return Err(HookError::RemoteRefused);
    }
    Ok(())
}

/// Install hooks for the configured framework, auto-detecting if the
/// config doesn't pin one. Returns `Ok(true)` when hooks were installed,
/// `Ok(false)` when nothing was configured / detected. Errors are
/// advisory in the setup flow — callers map to a warning, not a fatal
/// exit.
///
/// Emits `git_hooks.install_started` / `git_hooks.install_completed`
/// envelopes (obs P1, review items 23 + 24) so the CLI entry points
/// (`jarvy hooks install`) carry the same structured-event signal that
/// the setup-time phase wrapper does. Distinct from the run-level
/// `git_hooks.phase_*` envelopes in `setup_cmd::run_git_hooks_phase`
/// (which wrap the full install + auto_update + run_after_install
/// pipeline).
pub fn install_hooks(config: &GitHooksConfig, project_dir: &Path) -> Result<bool, HookError> {
    let telemetry_on = crate::observability::telemetry_gate::is_enabled();
    let started = std::time::Instant::now();
    if telemetry_on {
        tracing::info!(
            event = "git_hooks.install_started",
            enabled = config.enabled,
            auto_update = config.auto_update,
            run_after_install = config.run_after_install,
        );
    }
    let outcome = install_hooks_inner(config, project_dir);
    if telemetry_on {
        let (status, applied, framework_label) = match &outcome {
            Ok(true) => (
                "applied",
                true,
                config
                    .framework
                    .or_else(|| detect_framework(project_dir))
                    .map(HookFramework::as_str)
                    .unwrap_or("none"),
            ),
            Ok(false) => ("skipped", false, "none"),
            Err(_) => ("failed", false, "none"),
        };
        tracing::info!(
            event = "git_hooks.install_completed",
            status = status,
            applied = applied,
            framework = framework_label,
            auto_update = config.auto_update,
            run_after_install = config.run_after_install,
            duration_ms = started.elapsed().as_millis() as u64,
        );
    }
    outcome
}

fn install_hooks_inner(config: &GitHooksConfig, project_dir: &Path) -> Result<bool, HookError> {
    if !config.enabled {
        return Ok(false);
    }
    enforce_remote_gate(config)?;
    if !project_dir.join(".git").exists() {
        return Err(HookError::NotAGitRepo);
    }

    let framework = match config.framework.or_else(|| detect_framework(project_dir)) {
        Some(f) => f,
        None => return Ok(false),
    };

    match framework {
        HookFramework::Husky => {
            let handler = husky::HuskyHandler::new(project_dir.to_path_buf());
            handler.install()?;
            Ok(true)
        }
        HookFramework::Lefthook => {
            let handler = lefthook::LefthookHandler::new(project_dir.to_path_buf());
            handler.install()?;
            Ok(true)
        }
        HookFramework::Native => {
            let handler = native::NativeHandler::new(
                config.native.clone().unwrap_or_default(),
                project_dir.to_path_buf(),
            );
            handler.install()?;
            Ok(true)
        }
    }
}

/// Update hooks (currently: pre-commit autoupdate). Behavior parallels
/// `install_hooks` — Ok(true) on update, Ok(false) when nothing to do.
pub fn update_hooks(config: &GitHooksConfig, project_dir: &Path) -> Result<bool, HookError> {
    let telemetry_on = crate::observability::telemetry_gate::is_enabled();
    let started = std::time::Instant::now();
    if telemetry_on {
        tracing::info!(event = "git_hooks.update_started", enabled = config.enabled,);
    }
    let outcome = update_hooks_inner(config, project_dir);
    if telemetry_on {
        let (status, applied, framework_label) = match &outcome {
            Ok(true) => (
                "applied",
                true,
                config
                    .framework
                    .or_else(|| detect_framework(project_dir))
                    .map(HookFramework::as_str)
                    .unwrap_or("none"),
            ),
            Ok(false) => ("skipped", false, "none"),
            Err(_) => ("failed", false, "none"),
        };
        tracing::info!(
            event = "git_hooks.update_completed",
            status = status,
            applied = applied,
            framework = framework_label,
            duration_ms = started.elapsed().as_millis() as u64,
        );
    }
    outcome
}

fn update_hooks_inner(config: &GitHooksConfig, project_dir: &Path) -> Result<bool, HookError> {
    if !config.enabled {
        return Ok(false);
    }
    enforce_remote_gate(config)?;
    let framework = match config.framework.or_else(|| detect_framework(project_dir)) {
        Some(f) => f,
        None => return Ok(false),
    };
    match framework {
        HookFramework::Husky => {
            let handler = husky::HuskyHandler::new(project_dir.to_path_buf());
            handler.update()?;
            Ok(true)
        }
        HookFramework::Lefthook => {
            let handler = lefthook::LefthookHandler::new(project_dir.to_path_buf());
            handler.update()?;
            Ok(true)
        }
        HookFramework::Native => {
            let handler = native::NativeHandler::new(
                config.native.clone().unwrap_or_default(),
                project_dir.to_path_buf(),
            );
            handler.update()?;
            Ok(true)
        }
    }
}

/// List installed hooks by walking whichever handler is active.
pub fn list_hooks(config: &GitHooksConfig, project_dir: &Path) -> Result<Vec<HookInfo>, HookError> {
    let framework = match config.framework.or_else(|| detect_framework(project_dir)) {
        Some(f) => f,
        None => return Ok(Vec::new()),
    };
    match framework {
        HookFramework::Husky => {
            let handler = husky::HuskyHandler::new(project_dir.to_path_buf());
            handler.list()
        }
        HookFramework::Lefthook => {
            let handler = lefthook::LefthookHandler::new(project_dir.to_path_buf());
            handler.list()
        }
        HookFramework::Native => {
            let handler = native::NativeHandler::new(
                config.native.clone().unwrap_or_default(),
                project_dir.to_path_buf(),
            );
            handler.list()
        }
    }
}

/// Run hooks once. `all_files = true` runs every configured hook
/// against the whole tree; `hook_id = Some("...")` runs a single stage.
pub fn run_hooks(
    config: &GitHooksConfig,
    project_dir: &Path,
    all_files: bool,
    hook_id: Option<&str>,
) -> Result<(), HookError> {
    enforce_remote_gate(config)?;
    let framework = match config.framework.or_else(|| detect_framework(project_dir)) {
        Some(f) => f,
        None => {
            return Err(HookError::Config(
                "no hook framework detected; nothing to run".to_string(),
            ));
        }
    };
    match framework {
        HookFramework::Husky => {
            let handler = husky::HuskyHandler::new(project_dir.to_path_buf());
            handler.run(all_files, hook_id)
        }
        HookFramework::Lefthook => {
            let handler = lefthook::LefthookHandler::new(project_dir.to_path_buf());
            handler.run(all_files, hook_id)
        }
        HookFramework::Native => {
            let handler = native::NativeHandler::new(
                config.native.clone().unwrap_or_default(),
                project_dir.to_path_buf(),
            );
            handler.run(all_files, hook_id)
        }
    }
}

/// Hook installation status — what `jarvy hooks status` returns.
#[derive(Debug, Clone)]
pub struct HookStatus {
    pub framework: Option<HookFramework>,
    pub installed: bool,
    pub config_path: Option<String>,
    pub hook_count: usize,
}

/// Probe current status: framework detected? any hook in `.git/hooks/`?
pub fn hook_status(config: &GitHooksConfig, project_dir: &Path) -> HookStatus {
    let framework = config.framework.or_else(|| detect_framework(project_dir));
    // Presence of ANY managed hook file in `.git/hooks/` counts as
    // installed — pre-commit's hardcoded probe was a leftover from
    // when it was the only framework.
    let hooks_dir = project_dir.join(".git").join("hooks");
    let installed = list_hooks(config, project_dir)
        .map(|list| list.iter().any(|h| hooks_dir.join(&h.id).exists()))
        .unwrap_or(false);
    let hook_count = list_hooks(config, project_dir)
        .map(|h| h.len())
        .unwrap_or(0);
    HookStatus {
        framework,
        installed,
        config_path: None,
        hook_count,
    }
}

/// A single hook entry surfaced by `jarvy hooks list`.
///
/// `hook_type` is reserved for non-pre-commit frameworks that
/// distinguish hook stages (commit-msg, pre-push, etc.). Today every
/// emitted value is `"pre-commit"` — the field exists so adding husky /
/// lefthook later doesn't require a breaking struct change.
#[derive(Debug, Clone)]
pub struct HookInfo {
    pub id: String,
    pub repo: String,
    pub version: String,
    #[allow(dead_code)] // Reserved for husky/lefthook handlers
    pub hook_type: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_hooks::ConfigOrigin;
    use tempfile::tempdir;

    /// Review item 5 (P0). Remote-origin config with default
    /// `allow_remote = false` must refuse install / update / run.
    #[test]
    fn install_hooks_refuses_remote_without_allow_remote_opt_in() {
        let tmp = tempdir().unwrap();
        std::fs::create_dir(tmp.path().join(".git")).unwrap();
        let cfg = GitHooksConfig {
            enabled: true,
            framework: Some(HookFramework::Native),
            auto_install: true,
            auto_update: false,
            run_after_install: false,
            allow_remote: false,
            native: None,
            origin: ConfigOrigin::Remote,
        };
        let err = install_hooks(&cfg, tmp.path()).expect_err("remote must refuse");
        assert!(matches!(err, HookError::RemoteRefused), "got {err:?}");
    }

    #[test]
    fn update_hooks_refuses_remote_without_allow_remote_opt_in() {
        let tmp = tempdir().unwrap();
        std::fs::create_dir(tmp.path().join(".git")).unwrap();
        let cfg = GitHooksConfig {
            enabled: true,
            framework: Some(HookFramework::Native),
            auto_install: true,
            auto_update: false,
            run_after_install: false,
            allow_remote: false,
            native: None,
            origin: ConfigOrigin::Remote,
        };
        let err = update_hooks(&cfg, tmp.path()).expect_err("remote must refuse");
        assert!(matches!(err, HookError::RemoteRefused));
    }

    #[test]
    fn run_hooks_refuses_remote_without_allow_remote_opt_in() {
        let tmp = tempdir().unwrap();
        std::fs::create_dir(tmp.path().join(".git")).unwrap();
        let cfg = GitHooksConfig {
            enabled: true,
            framework: Some(HookFramework::Native),
            auto_install: true,
            auto_update: false,
            run_after_install: false,
            allow_remote: false,
            native: None,
            origin: ConfigOrigin::Remote,
        };
        let err = run_hooks(&cfg, tmp.path(), false, None).expect_err("remote must refuse");
        assert!(matches!(err, HookError::RemoteRefused));
    }

    /// Remote-origin config WITH explicit `allow_remote = true` passes
    /// the gate (proceeds to framework detection / handler).
    #[test]
    fn install_hooks_accepts_remote_when_explicitly_opted_in() {
        let tmp = tempdir().unwrap();
        // No .git dir → install_hooks returns NotAGitRepo AFTER the
        // gate check passes. That proves the gate didn't fire.
        let cfg = GitHooksConfig {
            enabled: true,
            framework: Some(HookFramework::Native),
            auto_install: true,
            auto_update: false,
            run_after_install: false,
            allow_remote: true,
            native: None,
            origin: ConfigOrigin::Remote,
        };
        let err =
            install_hooks(&cfg, tmp.path()).expect_err("no .git → NotAGitRepo, NOT RemoteRefused");
        assert!(
            matches!(err, HookError::NotAGitRepo),
            "expected gate to pass + later check to fail with NotAGitRepo; got {err:?}"
        );
    }

    /// Local-origin config (default) is unchanged — no `allow_remote`
    /// needed.
    #[test]
    fn install_hooks_local_origin_unchanged() {
        let tmp = tempdir().unwrap();
        let cfg = GitHooksConfig {
            enabled: true,
            framework: Some(HookFramework::Native),
            auto_install: true,
            auto_update: false,
            run_after_install: false,
            allow_remote: false,
            native: None,
            origin: ConfigOrigin::Local, // default
        };
        let err = install_hooks(&cfg, tmp.path()).expect_err("no .git → NotAGitRepo");
        assert!(matches!(err, HookError::NotAGitRepo));
    }
}
