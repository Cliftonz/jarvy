//! Git hook framework installation (PRD-048)
//!
//! Installs and manages Git pre-commit hooks driven by `jarvy.toml`'s
//! `[git_hooks]` block. Today only the `pre-commit` framework
//! (<https://pre-commit.com>) is supported; the architecture leaves room
//! for `husky` and `lefthook` handlers behind the same `HookFramework`
//! enum without changing the CLI surface.
//!
//! # Why `[git_hooks]` and not `[hooks]`
//!
//! `[hooks]` is already used by `jarvy setup` for `pre_setup` /
//! `post_install` / `post_setup` shell scripts (PRD-003). Adding a
//! `git_hooks = true` knob into that existing block would entangle two
//! unrelated lifecycles. Using a new top-level `[git_hooks]` keeps
//! their semantics independent and lets users mix-and-match (no setup
//! hooks but yes pre-commit, or vice versa).
//!
//! # Trust boundary
//!
//! Pre-commit configs (`.pre-commit-config.yaml`) reference hook repos
//! by URL + revision. `jarvy hooks install` will fetch and execute
//! arbitrary code from those repos at commit time — same trust model as
//! `pre-commit install` itself. Jarvy does NOT add an additional gate
//! here because (a) the user must already trust the repo they're
//! working in, and (b) pre-commit's own `--hook-impl` sandboxing is
//! upstream's responsibility. Remote configs fetched via
//! `jarvy setup --from <url>` are blocked from auto-installing hooks
//! unless `[git_hooks] allow_remote = true` is set in the SOURCE config
//! (mirrors `[packages] allow_remote`).

pub mod config;
pub mod detection;
pub mod precommit;

use std::path::Path;
use thiserror::Error;

#[allow(unused_imports)] // Public re-export for downstream consumers
pub use config::PreCommitConfig;
pub use config::{GitHooksConfig, HookFramework};
pub use detection::detect_framework;
pub use precommit::PreCommitHandler;

/// Errors produced by hook installation / management.
#[derive(Debug, Error)]
pub enum HookError {
    #[error("hook framework `{0}` is not installed; install it before running `jarvy hooks`")]
    FrameworkNotInstalled(String),

    #[error("hook framework `{0}` is configured but not yet supported by jarvy")]
    UnsupportedFramework(String),

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
            HookError::NotAGitRepo => "not_a_git_repo",
            HookError::InstallFailed(_) => "install_failed",
            HookError::UpdateFailed(_) => "update_failed",
            HookError::RunFailed(_) => "run_failed",
            HookError::Config(_) => "config",
            HookError::Io(_) => "io",
        }
    }
}

/// Install hooks for the configured framework, auto-detecting if the
/// config doesn't pin one. Returns `Ok(true)` when hooks were installed,
/// `Ok(false)` when nothing was configured / detected. Errors are
/// advisory in the setup flow — callers map to a warning, not a fatal
/// exit.
pub fn install_hooks(config: &GitHooksConfig, project_dir: &Path) -> Result<bool, HookError> {
    if !config.enabled {
        return Ok(false);
    }
    if !project_dir.join(".git").exists() {
        return Err(HookError::NotAGitRepo);
    }

    let framework = match config.framework.or_else(|| detect_framework(project_dir)) {
        Some(f) => f,
        None => return Ok(false),
    };

    match framework {
        HookFramework::PreCommit => {
            let handler = PreCommitHandler::new(
                config.pre_commit.clone().unwrap_or_default(),
                project_dir.to_path_buf(),
            );
            handler.install()?;
            Ok(true)
        }
        HookFramework::Husky | HookFramework::Lefthook | HookFramework::Native => Err(
            HookError::UnsupportedFramework(framework.as_str().to_string()),
        ),
    }
}

/// Update hooks (currently: pre-commit autoupdate). Behavior parallels
/// `install_hooks` — Ok(true) on update, Ok(false) when nothing to do.
pub fn update_hooks(config: &GitHooksConfig, project_dir: &Path) -> Result<bool, HookError> {
    if !config.enabled {
        return Ok(false);
    }
    let framework = match config.framework.or_else(|| detect_framework(project_dir)) {
        Some(f) => f,
        None => return Ok(false),
    };
    match framework {
        HookFramework::PreCommit => {
            let handler = PreCommitHandler::new(
                config.pre_commit.clone().unwrap_or_default(),
                project_dir.to_path_buf(),
            );
            handler.update()?;
            Ok(true)
        }
        HookFramework::Husky | HookFramework::Lefthook | HookFramework::Native => Err(
            HookError::UnsupportedFramework(framework.as_str().to_string()),
        ),
    }
}

/// List installed hooks (currently: parse `.pre-commit-config.yaml`).
pub fn list_hooks(config: &GitHooksConfig, project_dir: &Path) -> Result<Vec<HookInfo>, HookError> {
    let framework = match config.framework.or_else(|| detect_framework(project_dir)) {
        Some(f) => f,
        None => return Ok(Vec::new()),
    };
    match framework {
        HookFramework::PreCommit => {
            let handler = PreCommitHandler::new(
                config.pre_commit.clone().unwrap_or_default(),
                project_dir.to_path_buf(),
            );
            handler.list()
        }
        HookFramework::Husky | HookFramework::Lefthook | HookFramework::Native => Err(
            HookError::UnsupportedFramework(framework.as_str().to_string()),
        ),
    }
}

/// Run hooks once. `all_files = true` mirrors `pre-commit run
/// --all-files`. `hook_id = Some("black")` runs a single hook.
pub fn run_hooks(
    config: &GitHooksConfig,
    project_dir: &Path,
    all_files: bool,
    hook_id: Option<&str>,
) -> Result<(), HookError> {
    let framework = match config.framework.or_else(|| detect_framework(project_dir)) {
        Some(f) => f,
        None => {
            return Err(HookError::Config(
                "no hook framework detected; nothing to run".to_string(),
            ));
        }
    };
    match framework {
        HookFramework::PreCommit => {
            let handler = PreCommitHandler::new(
                config.pre_commit.clone().unwrap_or_default(),
                project_dir.to_path_buf(),
            );
            handler.run(all_files, hook_id)
        }
        HookFramework::Husky | HookFramework::Lefthook | HookFramework::Native => Err(
            HookError::UnsupportedFramework(framework.as_str().to_string()),
        ),
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

/// Probe current status: framework detected? installed in `.git/hooks/`?
pub fn hook_status(config: &GitHooksConfig, project_dir: &Path) -> HookStatus {
    let framework = config.framework.or_else(|| detect_framework(project_dir));
    let installed = project_dir
        .join(".git")
        .join("hooks")
        .join("pre-commit")
        .exists();
    let (config_path, hook_count) = match framework {
        Some(HookFramework::PreCommit) => {
            let path = config
                .pre_commit
                .as_ref()
                .map(|c| c.config.clone())
                .unwrap_or_else(|| ".pre-commit-config.yaml".to_string());
            let count = if project_dir.join(&path).exists() {
                let handler = PreCommitHandler::new(
                    config.pre_commit.clone().unwrap_or_default(),
                    project_dir.to_path_buf(),
                );
                handler.list().map(|h| h.len()).unwrap_or(0)
            } else {
                0
            };
            (Some(path), count)
        }
        _ => (None, 0),
    };
    HookStatus {
        framework,
        installed,
        config_path,
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
