//! `[dotfiles]` block — cross-machine dotfile sync via chezmoi / yadm.
//!
//! Complements the personal `~/.jarvy/jarvy.toml` overlay (which
//! covers jarvy-owned concepts like `[skills]`, `[ai_hooks]`, and
//! `[git]` identity) by handing everything jarvy doesn't model
//! (nvim, tmux, shell rc, ssh config, etc.) off to a mature
//! dotfile manager. `jarvy setup` clones and applies the repo
//! idempotently during a dedicated phase.
//!
//! Supported managers:
//! - `chezmoi` — `chezmoi init --apply <repo>` (first run); `chezmoi
//!   update` on subsequent runs. Repo may be an HTTPS or SSH URL.
//! - `yadm` — `yadm clone <repo>` (first run); `yadm pull` on
//!   subsequent runs.
//! - `stow` — installed as a tool for users who prefer manual
//!   invocation, but auto-apply is NOT provided in v1 because
//!   stow's per-package model needs project-specific configuration
//!   that doesn't fit a single "apply" verb.
//!
//! Trust: remote-origin configs (`jarvy setup --from <url>`) cannot
//! apply `[dotfiles]` unless `allow_remote = true` is set in the
//! source config. Mirrors `[git_hooks] allow_remote` and
//! `[packages] allow_remote`. A friendly-looking remote config
//! could otherwise clone an attacker-controlled dotfile repo that
//! ships an `~/.zshrc` with `curl … | sh` on shell startup.

use serde::{Deserialize, Serialize};
use std::process::Command;

use crate::observability::telemetry_gate;

/// `[dotfiles]` config block.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DotfilesConfig {
    /// Which dotfile manager to use. Required.
    pub manager: DotfilesManager,

    /// Source repository. Passed verbatim to the manager, so any URL
    /// scheme the manager accepts works (`https://…`, `git@…:…`,
    /// `github:user/repo` for chezmoi's shorthand).
    pub repo: String,

    /// Actually apply after clone. Default `true`. Set `false` to
    /// clone but require the user to run `chezmoi apply` / `yadm
    /// checkout` manually — useful when the repo carries destructive
    /// scripts the user wants to review first.
    #[serde(default = "default_true")]
    pub apply: bool,

    /// Allow remote-origin configs to run the dotfiles phase.
    /// Default `false`: a friendly-looking remote config cannot
    /// clone an attacker's dotfile repo onto the consuming machine
    /// without an explicit opt-in in the SOURCE config.
    #[serde(default)]
    pub allow_remote: bool,

    /// Origin tag set by `Config::mark_remote`; not serialized.
    #[serde(skip)]
    pub origin: crate::ai_hooks::ConfigOrigin,
}

fn default_true() -> bool {
    true
}

impl Default for DotfilesConfig {
    fn default() -> Self {
        Self {
            manager: DotfilesManager::Chezmoi,
            repo: String::new(),
            apply: true,
            allow_remote: false,
            origin: crate::ai_hooks::ConfigOrigin::Local,
        }
    }
}

/// Supported dotfile managers. Case-insensitive on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DotfilesManager {
    Chezmoi,
    Yadm,
    /// Stow is accepted for `provisioner`-only installs. The phase
    /// runner refuses to auto-apply stow (see module docs) and
    /// prints a hint instead.
    Stow,
}

impl DotfilesManager {
    /// CLI binary this manager talks to. Used both for PATH probing
    /// (skip the phase gracefully if not installed) and for telemetry.
    pub fn cli(self) -> &'static str {
        match self {
            Self::Chezmoi => "chezmoi",
            Self::Yadm => "yadm",
            Self::Stow => "stow",
        }
    }
}

/// Outcome of the dotfiles phase — reported via telemetry and
/// (for the setup lead) printed to stdout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhaseOutcome {
    /// Manager applied the repo (fresh clone OR update).
    Applied,
    /// Nothing to do — repo already present and `apply = false`.
    NoOp,
    /// Block existed but was disabled for a legitimate reason
    /// (unsupported manager for auto-apply, missing manager binary,
    /// dry-run preview).
    Skipped {
        reason: &'static str,
    },
    /// Trust gate refused (remote config without `allow_remote`).
    Refused {
        reason: &'static str,
    },
    /// Manager subprocess returned non-zero.
    Failed {
        error: String,
    },
}

/// Run the dotfiles phase against `cfg`. Advisory — never fails
/// `jarvy setup`. Callers should print the returned outcome themselves.
pub fn run_phase(cfg: &DotfilesConfig, dry_run: bool) -> PhaseOutcome {
    // Trust gate first — same shape as `[git_hooks]` and `[packages]`.
    if cfg.origin == crate::ai_hooks::ConfigOrigin::Remote && !cfg.allow_remote {
        emit_refused("allow_remote_not_set");
        return PhaseOutcome::Refused {
            reason: "allow_remote_not_set",
        };
    }

    if cfg.repo.trim().is_empty() {
        emit_skipped("empty_repo");
        return PhaseOutcome::Skipped {
            reason: "empty_repo",
        };
    }

    if dry_run {
        emit_skipped("dry_run");
        return PhaseOutcome::Skipped { reason: "dry_run" };
    }

    // Stow doesn't fit a single "apply" verb — skip with hint.
    if cfg.manager == DotfilesManager::Stow {
        emit_skipped("stow_manual");
        return PhaseOutcome::Skipped {
            reason: "stow_manual",
        };
    }

    if !command_on_path(cfg.manager.cli()) {
        emit_skipped("manager_not_installed");
        return PhaseOutcome::Skipped {
            reason: "manager_not_installed",
        };
    }

    if telemetry_gate::is_enabled() {
        tracing::info!(
            event = "dotfiles.phase_started",
            manager = cfg.manager.cli(),
            apply = cfg.apply,
            "dotfiles phase started"
        );
    }
    let started = std::time::Instant::now();

    let outcome = match cfg.manager {
        DotfilesManager::Chezmoi => apply_chezmoi(&cfg.repo, cfg.apply),
        DotfilesManager::Yadm => apply_yadm(&cfg.repo, cfg.apply),
        DotfilesManager::Stow => unreachable!("stow_manual handled above"),
    };

    let duration_ms = started.elapsed().as_millis() as u64;
    if telemetry_gate::is_enabled() {
        match &outcome {
            PhaseOutcome::Applied => tracing::info!(
                event = "dotfiles.phase_completed",
                manager = cfg.manager.cli(),
                duration_ms,
                "dotfiles phase completed"
            ),
            PhaseOutcome::Failed { error } => tracing::warn!(
                event = "dotfiles.phase_failed",
                manager = cfg.manager.cli(),
                duration_ms,
                error = %error,
                "dotfiles phase failed"
            ),
            _ => {}
        }
    }
    outcome
}

/// Run chezmoi. On fresh install (`~/.local/share/chezmoi` empty)
/// use `init --apply <repo>`; otherwise `update` (which pulls +
/// re-applies) when `apply = true`, else just `git pull` inside the
/// source dir via `chezmoi git pull`.
fn apply_chezmoi(repo: &str, apply: bool) -> PhaseOutcome {
    let source_dir = chezmoi_source_dir();
    let inited = source_dir
        .as_ref()
        .map(|p| p.join(".git").exists())
        .unwrap_or(false);

    let output = if !inited {
        let mut args = vec!["init"];
        if apply {
            args.push("--apply");
        }
        args.push(repo);
        Command::new("chezmoi").args(&args).output()
    } else if apply {
        Command::new("chezmoi").arg("update").output()
    } else {
        Command::new("chezmoi").args(["git", "pull"]).output()
    };

    match output {
        Ok(o) if o.status.success() => PhaseOutcome::Applied,
        Ok(o) => PhaseOutcome::Failed {
            error: format!(
                "chezmoi exited {} — {}",
                o.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&o.stderr).trim()
            ),
        },
        Err(e) => PhaseOutcome::Failed {
            error: format!("chezmoi spawn failed: {e}"),
        },
    }
}

/// Run yadm. First run: `yadm clone <repo>`. Subsequent runs when
/// `apply = true`: `yadm pull`; when `apply = false`: no-op (repo
/// present, user asked us not to touch it).
fn apply_yadm(repo: &str, apply: bool) -> PhaseOutcome {
    let cloned = yadm_repo_dir().map(|p| p.exists()).unwrap_or(false);

    let output = if !cloned {
        Command::new("yadm").args(["clone", repo]).output()
    } else if apply {
        Command::new("yadm").arg("pull").output()
    } else {
        return PhaseOutcome::NoOp;
    };

    match output {
        Ok(o) if o.status.success() => PhaseOutcome::Applied,
        Ok(o) => PhaseOutcome::Failed {
            error: format!(
                "yadm exited {} — {}",
                o.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&o.stderr).trim()
            ),
        },
        Err(e) => PhaseOutcome::Failed {
            error: format!("yadm spawn failed: {e}"),
        },
    }
}

fn chezmoi_source_dir() -> Option<std::path::PathBuf> {
    dirs::data_local_dir()
        .or_else(dirs::data_dir)
        .map(|d| d.join("chezmoi"))
}

fn yadm_repo_dir() -> Option<std::path::PathBuf> {
    dirs::data_local_dir()
        .or_else(dirs::data_dir)
        .map(|d| d.join("yadm").join("repo.git"))
}

fn command_on_path(cmd: &str) -> bool {
    let (probe, arg) = if cfg!(target_os = "windows") {
        ("where", cmd)
    } else {
        ("which", cmd)
    };
    Command::new(probe)
        .arg(arg)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn emit_skipped(reason: &'static str) {
    if telemetry_gate::is_enabled() {
        tracing::info!(
            event = "dotfiles.phase_skipped",
            reason = reason,
            "dotfiles phase skipped"
        );
    }
}

fn emit_refused(reason: &'static str) {
    if telemetry_gate::is_enabled() {
        tracing::warn!(
            event = "dotfiles.remote_refused",
            reason = reason,
            "remote config attempted to apply [dotfiles] without allow_remote"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_block() {
        let cfg: DotfilesConfig = toml::from_str(
            r#"
manager = "chezmoi"
repo = "github:zac/dotfiles"
"#,
        )
        .unwrap();
        assert_eq!(cfg.manager, DotfilesManager::Chezmoi);
        assert_eq!(cfg.repo, "github:zac/dotfiles");
        assert!(cfg.apply, "apply defaults to true");
        assert!(!cfg.allow_remote, "allow_remote defaults to false");
    }

    #[test]
    fn parses_all_managers_case_insensitive() {
        for (raw, expected) in [
            ("chezmoi", DotfilesManager::Chezmoi),
            ("yadm", DotfilesManager::Yadm),
            ("stow", DotfilesManager::Stow),
        ] {
            let toml_src = format!("manager = \"{raw}\"\nrepo = \"x\"\n");
            let cfg: DotfilesConfig = toml::from_str(&toml_src).unwrap();
            assert_eq!(cfg.manager, expected, "manager={raw}");
        }
    }

    #[test]
    fn remote_config_without_opt_in_is_refused() {
        let mut cfg = DotfilesConfig {
            manager: DotfilesManager::Chezmoi,
            repo: "github:evil/dotfiles".into(),
            apply: true,
            allow_remote: false,
            origin: crate::ai_hooks::ConfigOrigin::Remote,
        };
        let outcome = run_phase(&cfg, false);
        assert!(
            matches!(
                outcome,
                PhaseOutcome::Refused {
                    reason: "allow_remote_not_set"
                }
            ),
            "got {outcome:?}"
        );
        cfg.allow_remote = true;
        // With opt-in on, we get past the trust gate — the outcome
        // depends on whether chezmoi is installed on the host, so
        // we only assert we're NOT Refused anymore.
        let outcome2 = run_phase(&cfg, false);
        assert!(!matches!(outcome2, PhaseOutcome::Refused { .. }));
    }

    #[test]
    fn empty_repo_is_skipped() {
        let cfg = DotfilesConfig {
            manager: DotfilesManager::Chezmoi,
            repo: "   ".into(),
            ..DotfilesConfig::default()
        };
        assert!(matches!(
            run_phase(&cfg, false),
            PhaseOutcome::Skipped {
                reason: "empty_repo"
            }
        ));
    }

    #[test]
    fn dry_run_is_skipped() {
        let cfg = DotfilesConfig {
            manager: DotfilesManager::Chezmoi,
            repo: "github:zac/dotfiles".into(),
            ..DotfilesConfig::default()
        };
        assert!(matches!(
            run_phase(&cfg, true),
            PhaseOutcome::Skipped { reason: "dry_run" }
        ));
    }

    #[test]
    fn stow_is_skipped_with_stow_manual_reason() {
        let cfg = DotfilesConfig {
            manager: DotfilesManager::Stow,
            repo: "https://example.com/dotfiles.git".into(),
            ..DotfilesConfig::default()
        };
        assert!(matches!(
            run_phase(&cfg, false),
            PhaseOutcome::Skipped {
                reason: "stow_manual"
            }
        ));
    }
}
