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

impl crate::ai_hooks::HasOrigin for DotfilesConfig {
    fn set_origin(&mut self, origin: crate::ai_hooks::ConfigOrigin) {
        self.origin = origin;
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

/// Which apply-flavor a success path actually took — carried on
/// `PhaseOutcome::Applied` and emitted as
/// `dotfiles.phase_completed.apply_kind`. Retention analytics needs
/// this: "initial_clone" vs "update" are entirely different funnel
/// states (adoption vs retention), and lumping them together hides
/// spikes in fleet re-provisioning (which can be a security
/// signal — dotfiles cloned from a compromised remote config).
///
/// Cardinality: 4. Bounded string enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyKind {
    /// chezmoi init --apply / yadm clone — first-time provisioning.
    InitialClone,
    /// chezmoi update — pull + re-apply on an already-inited source.
    Update,
    /// chezmoi git pull — pull only (apply = false path).
    GitPullOnly,
    /// yadm pull — pull only (already-cloned).
    Pull,
}

impl ApplyKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::InitialClone => "initial_clone",
            Self::Update => "update",
            Self::GitPullOnly => "git_pull_only",
            Self::Pull => "pull",
        }
    }
}

/// Bounded reasons the dotfiles phase can be `Skipped`. Replaces an
/// earlier `&'static str` field that was matched on at the setup_cmd
/// boundary — same rule declared in two files, silently regressed on
/// rename. Compiler now catches drift.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    /// `dry_run = true` in setup — never touch the manager.
    DryRun,
    /// `[dotfiles] repo = ""` (whitespace-only).
    EmptyRepo,
    /// `manager = "stow"` — no auto-apply path, user must invoke stow
    /// per-package by hand.
    StowManual,
    /// Manager binary not on PATH — user should add it under
    /// `[provisioner]` so jarvy installs it before this phase runs.
    ManagerNotInstalled,
}

impl SkipReason {
    /// Telemetry label — bounded &'static str for
    /// `dotfiles.phase_skipped.reason`. Kept stable so downstream
    /// dashboards don't break when we rename the enum variants.
    pub fn telemetry(self) -> &'static str {
        match self {
            Self::DryRun => "dry_run",
            Self::EmptyRepo => "empty_repo",
            Self::StowManual => "stow_manual",
            Self::ManagerNotInstalled => "manager_not_installed",
        }
    }
}

/// Outcome of the dotfiles phase — reported via telemetry and
/// (for the setup lead) printed to stdout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhaseOutcome {
    /// Manager applied the repo. `kind` distinguishes clone vs
    /// update so retention analytics can separate adoption from
    /// re-use.
    Applied {
        kind: ApplyKind,
    },
    /// Nothing to do — repo already present and `apply = false`.
    NoOp,
    /// Block existed but was disabled for a legitimate reason.
    Skipped(SkipReason),
    /// Trust gate refused (remote config without `allow_remote`)
    /// OR input-safety refused (leading-`-` repo, NUL byte).
    Refused {
        reason: &'static str,
    },
    /// Manager subprocess returned non-zero. `error_kind` is a
    /// bounded taxonomy safe for telemetry (never contains user-
    /// authored URL fragments or subprocess stderr). `error` is the
    /// full human-facing message; callers may print it locally but
    /// MUST NOT emit it as a telemetry field — subprocess stderr
    /// commonly echoes back the repo URL including embedded auth
    /// tokens (`https://x-oauth-token:ghp_XXXX@github.com/...`).
    Failed {
        error_kind: &'static str,
        error: String,
    },
}

/// Run the dotfiles phase against `cfg`. Advisory — never fails
/// `jarvy setup`. Callers should print the returned outcome themselves.
pub fn run_phase(cfg: &DotfilesConfig, dry_run: bool) -> PhaseOutcome {
    // Trust gate first — same shape as `[git_hooks]` and `[packages]`.
    if cfg.origin == crate::ai_hooks::ConfigOrigin::Remote && !cfg.allow_remote {
        emit_refused(cfg.manager.cli(), "allow_remote_not_set");
        return PhaseOutcome::Refused {
            reason: "allow_remote_not_set",
        };
    }

    if cfg.repo.trim().is_empty() {
        return skip(cfg.manager.cli(), SkipReason::EmptyRepo);
    }

    // Argv-safety gate: refuse repos with a leading `-` or NUL byte.
    // git treats `--upload-pack=/tmp/pwn` (CVE-2017-1000117 class) as
    // an option — passing it as a "positional" argument executes the
    // named binary as the upload-pack helper. Chezmoi has its own
    // flag surface (`--source`, `--exclude`, ...) with the same risk.
    // Command::args just tokenizes — the safety must live here.
    if !valid_repo_arg(&cfg.repo) {
        emit_refused(cfg.manager.cli(), "invalid_repo");
        return PhaseOutcome::Refused {
            reason: "invalid_repo",
        };
    }

    if dry_run {
        return skip(cfg.manager.cli(), SkipReason::DryRun);
    }

    // Stow doesn't fit a single "apply" verb — skip with hint.
    if cfg.manager == DotfilesManager::Stow {
        return skip(cfg.manager.cli(), SkipReason::StowManual);
    }

    if !command_on_path(cfg.manager.cli()) {
        return skip(cfg.manager.cli(), SkipReason::ManagerNotInstalled);
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
    let apply_mode = if cfg.apply { "auto_apply" } else { "clone_only" };
    if telemetry_gate::is_enabled() {
        match &outcome {
            PhaseOutcome::Applied { kind } => tracing::info!(
                event = "dotfiles.phase_completed",
                manager = cfg.manager.cli(),
                apply_kind = kind.as_str(),
                apply_mode,
                duration_ms,
                "dotfiles phase completed"
            ),
            PhaseOutcome::NoOp => tracing::info!(
                event = "dotfiles.phase_completed",
                manager = cfg.manager.cli(),
                apply_kind = "noop",
                apply_mode,
                duration_ms,
                "dotfiles phase noop (clone_only + already-present)"
            ),
            PhaseOutcome::Failed { error_kind, .. } => tracing::warn!(
                event = "dotfiles.phase_failed",
                manager = cfg.manager.cli(),
                apply_mode,
                duration_ms,
                error_kind = %error_kind,
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

    let (output, kind) = if !inited {
        let mut args = vec!["init"];
        if apply {
            args.push("--apply");
        }
        args.push("--");
        args.push(repo);
        (
            dotfile_command("chezmoi").args(&args).output(),
            ApplyKind::InitialClone,
        )
    } else if apply {
        (
            dotfile_command("chezmoi").arg("update").output(),
            ApplyKind::Update,
        )
    } else {
        (
            dotfile_command("chezmoi").args(["git", "pull"]).output(),
            ApplyKind::GitPullOnly,
        )
    };

    map_subprocess_output("chezmoi", output, kind)
}

/// Run yadm. First run: `yadm clone <repo>`. Subsequent runs when
/// `apply = true`: `yadm pull`; when `apply = false`: no-op (repo
/// present, user asked us not to touch it).
fn apply_yadm(repo: &str, apply: bool) -> PhaseOutcome {
    let cloned = yadm_repo_dir().map(|p| p.exists()).unwrap_or(false);

    let (output, kind) = if !cloned {
        (
            dotfile_command("yadm").args(["clone", "--", repo]).output(),
            ApplyKind::InitialClone,
        )
    } else if apply {
        (
            dotfile_command("yadm").arg("pull").output(),
            ApplyKind::Pull,
        )
    } else {
        return PhaseOutcome::NoOp;
    };

    map_subprocess_output("yadm", output, kind)
}

/// Shared tail: turn a subprocess result into a `PhaseOutcome`. Uses
/// [`classify_dotfiles_error`] so telemetry sees only a bounded
/// `error_kind` — the raw stderr (which typically carries the repo
/// URL with any embedded token) stays in the human-facing `error`
/// string for the setup lead's stderr line and never reaches OTLP.
fn map_subprocess_output(
    cli: &'static str,
    r: std::io::Result<std::process::Output>,
    kind: ApplyKind,
) -> PhaseOutcome {
    match r {
        Ok(o) if o.status.success() => PhaseOutcome::Applied { kind },
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            let error_kind = classify_dotfiles_error(&stderr);
            PhaseOutcome::Failed {
                error_kind,
                error: format!(
                    "{cli} exited {} — {}",
                    o.status.code().unwrap_or(-1),
                    stderr.trim()
                ),
            }
        }
        Err(e) => PhaseOutcome::Failed {
            error_kind: match e.kind() {
                std::io::ErrorKind::NotFound => "binary_missing",
                std::io::ErrorKind::PermissionDenied => "permission_denied",
                _ => "spawn_failed",
            },
            error: format!("{cli} spawn failed: {e}"),
        },
    }
}

/// Bounded taxonomy of chezmoi / yadm (git-driven) failure classes.
/// Keeps `dotfiles.phase_failed { error_kind }` a small, alertable
/// enum so PMs and SREs can graph by cause without cardinality bombs
/// and without letting user-authored repo URLs leak into the event
/// stream.
///
/// Cardinality: 5. Any new class MUST be added here explicitly.
fn classify_dotfiles_error(stderr: &str) -> &'static str {
    // Auth failures — the most common leak vector, and the class git
    // most likes to echo the full URL back on.
    if stderr.contains("Authentication failed")
        || stderr.contains("could not read Username")
        || stderr.contains("Permission denied (publickey)")
        || stderr.contains("Host key verification failed")
    {
        return "auth";
    }
    // Network / DNS.
    if stderr.contains("Could not resolve host")
        || stderr.contains("Connection timed out")
        || stderr.contains("Connection refused")
        || stderr.contains("Network is unreachable")
    {
        return "network";
    }
    // Target doesn't exist.
    if stderr.contains("Repository not found")
        || stderr.contains("does not exist")
        || stderr.contains("not found")
    {
        return "not_found";
    }
    // Local file / git-state conflict (untracked file collision, etc).
    if stderr.contains("would be overwritten") || stderr.contains("conflict") {
        return "conflict";
    }
    "other"
}

/// Validate a `[dotfiles] repo` value before handing it to chezmoi /
/// yadm as a positional argument. Refuses:
///
/// - empty / whitespace-only,
/// - leading `-` (option-injection into git's argv — CVE-2017-1000117
///   pattern; git's `--upload-pack=/tmp/pwn` executes an arbitrary
///   local binary as the upload-pack helper),
/// - embedded NUL byte (defense-in-depth; `Command::args` accepts
///   `&str` which cannot contain NUL, but a `String` from TOML can
///   round-trip via `OsString` → panics).
///
/// Passing `--` before the repo in the argv is belt-and-suspenders —
/// modern git honors it, older git and third-party wrappers may not.
fn valid_repo_arg(repo: &str) -> bool {
    let r = repo.trim();
    !r.is_empty() && !r.starts_with('-') && !r.contains('\0')
}

/// Build a `Command` with GIT_*/CHEZMOI_* code-execution env vars
/// scrubbed. Attacker with prior env influence (`.bashrc` line the
/// user added years ago, `sudo -E`, tainted CI job) would otherwise
/// execute code the moment `jarvy setup` reaches the dotfiles phase:
///
/// - `GIT_SSH_COMMAND` / `GIT_ASKPASS` — arbitrary program to run per
///   git remote op.
/// - `GIT_EXTERNAL_DIFF` / `GIT_PAGER` — arbitrary program for diff /
///   paging inside the clone/apply.
/// - `GIT_PROXY_COMMAND` — arbitrary program invoked to reach the
///   remote host.
/// - `CHEZMOI_CONFIG_FILE` / `CHEZMOI_SOURCE_DIR` — redirect chezmoi
///   at attacker-controlled config / source tree.
fn dotfile_command(bin: &str) -> Command {
    let mut cmd = Command::new(bin);
    for var in [
        "GIT_SSH_COMMAND",
        "GIT_EXTERNAL_DIFF",
        "GIT_PAGER",
        "GIT_ASKPASS",
        "GIT_PROXY_COMMAND",
        "CHEZMOI_CONFIG_FILE",
        "CHEZMOI_SOURCE_DIR",
    ] {
        cmd.env_remove(var);
    }
    cmd
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
    crate::tools::common::command_on_path(cmd)
}

/// Convenience: emit `dotfiles.phase_skipped` and return the matching
/// `PhaseOutcome::Skipped(SkipReason)` in one call. Callers get compile-
/// time-checked SkipReason arms; the telemetry label is derived from the
/// enum so it can't drift.
fn skip(manager: &'static str, reason: SkipReason) -> PhaseOutcome {
    if telemetry_gate::is_enabled() {
        tracing::info!(
            event = "dotfiles.phase_skipped",
            manager,
            reason = reason.telemetry(),
            "dotfiles phase skipped"
        );
    }
    PhaseOutcome::Skipped(reason)
}

fn emit_refused(manager: &'static str, reason: &'static str) {
    if telemetry_gate::is_enabled() {
        tracing::warn!(
            event = "dotfiles.remote_refused",
            manager,
            reason,
            "[dotfiles] refused (remote trust gate or input-safety gate)"
        );
    }
}

/// Emitted when `[dotfiles]` is absent from `jarvy.toml`. Fires
/// once per `jarvy setup` invocation regardless of overlay state.
/// This is the denominator for "of setup runs, how many are eligible
/// to adopt the dotfiles feature?" — without it, adoption rate is
/// unbounded (the numerator, `dotfiles.phase_*`, has no base).
pub fn emit_phase_absent() {
    if telemetry_gate::is_enabled() {
        tracing::debug!(
            event = "dotfiles.phase_absent",
            "no [dotfiles] block in project config"
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
            PhaseOutcome::Skipped(SkipReason::EmptyRepo)
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
            PhaseOutcome::Skipped(SkipReason::DryRun)
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
            PhaseOutcome::Skipped(SkipReason::StowManual)
        ));
    }

    /// A `repo` starting with `-` is argv-injection into git (CVE-2017-1000117
    /// pattern — `--upload-pack=/tmp/pwn` executes an arbitrary binary as the
    /// upload-pack helper). Must be refused before we ever spawn.
    #[test]
    fn leading_dash_repo_is_refused_without_spawning() {
        for hostile in [
            "--upload-pack=/tmp/pwn",
            "--source=/tmp/evil",
            "-oProxyCommand=/tmp/x",
        ] {
            let cfg = DotfilesConfig {
                manager: DotfilesManager::Chezmoi,
                repo: hostile.into(),
                ..DotfilesConfig::default()
            };
            assert!(
                matches!(
                    run_phase(&cfg, false),
                    PhaseOutcome::Refused {
                        reason: "invalid_repo"
                    }
                ),
                "hostile repo `{hostile}` should be refused"
            );
        }
    }

    #[test]
    fn nul_byte_repo_is_refused() {
        let cfg = DotfilesConfig {
            manager: DotfilesManager::Chezmoi,
            repo: "github:x/y\0evil".into(),
            ..DotfilesConfig::default()
        };
        assert!(matches!(
            run_phase(&cfg, false),
            PhaseOutcome::Refused {
                reason: "invalid_repo"
            }
        ));
    }

    #[test]
    fn valid_repo_arg_accepts_common_shapes() {
        for ok in [
            "github:zac/dotfiles",
            "https://github.com/zac/dotfiles.git",
            "git@github.com:zac/dotfiles.git",
            "ssh://git@example.com/zac/dotfiles.git",
        ] {
            assert!(valid_repo_arg(ok), "should accept: {ok}");
        }
    }

    /// Bounded taxonomy — every case that reaches production stderr
    /// should map to a stable, non-URL-bearing string.
    #[test]
    fn classify_dotfiles_error_never_returns_raw_stderr() {
        let cases: &[(&str, &str)] = &[
            (
                "fatal: Authentication failed for 'https://x:ghp_XXX@github.com/acme/private.git/'",
                "auth",
            ),
            (
                "Permission denied (publickey).\nfatal: Could not read from remote repository.",
                "auth",
            ),
            ("ssh: Could not resolve host: github.com", "network"),
            (
                "fatal: repository 'https://github.com/acme/missing.git/' not found",
                "not_found",
            ),
            ("error: Your local changes would be overwritten by merge", "conflict"),
            ("chezmoi: some unknown message", "other"),
        ];
        for (stderr, expected_kind) in cases {
            assert_eq!(classify_dotfiles_error(stderr), *expected_kind, "stderr={stderr}");
        }
    }

    /// `PhaseOutcome::Failed { error_kind }` is a bounded &'static str;
    /// `error` (the human message) is separate. This test locks in that
    /// the *field name* used for telemetry is `error_kind`, not `error`,
    /// so a future refactor that stringifies the whole outcome can't
    /// silently start leaking again.
    #[test]
    fn phase_failed_carries_bounded_error_kind() {
        let f = PhaseOutcome::Failed {
            error_kind: "auth",
            error: "chezmoi exited 128 — fatal: Authentication failed for 'https://x:token@github.com/acme/private.git/'".into(),
        };
        if let PhaseOutcome::Failed { error_kind, error } = f {
            assert_eq!(error_kind, "auth");
            // The `error` field DOES carry the raw URL — that's the user-
            // facing message; telemetry must never read it.
            assert!(error.contains("token"), "raw stderr preserved for user");
        } else {
            unreachable!()
        }
    }
}
