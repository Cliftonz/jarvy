//! Shell initialization and ensure logic
//!
//! Provides two CLI features:
//! - `jarvy shell-init` — outputs an RC snippet for eval in shell profiles
//! - `jarvy ensure` — lightweight check-and-install for shell startup
//!
//! Configuration lives in `~/.jarvy/config.toml` under `[shell_init]`.
//! State is tracked in `~/.jarvy/ensure.stamp` to enable fast-path skipping.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::env::ShellType;
use crate::tools;

/// Configuration for shell init auto-ensure (in ~/.jarvy/config.toml)
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ShellInitConfig {
    /// Whether shell-init is enabled
    #[serde(default)]
    pub enabled: bool,
    /// Inline tool list to ensure on shell startup
    #[serde(default)]
    pub tools: Option<Vec<String>>,
    /// Version hints per tool
    #[serde(default)]
    pub versions: Option<HashMap<String, String>>,
    /// Run installation in background (default: true)
    #[serde(default = "default_true")]
    pub background: bool,
    /// Hours between re-checks (default: 24, 0 = every shell open)
    #[serde(default = "default_24")]
    pub check_interval: u64,
}

impl Default for ShellInitConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            tools: None,
            versions: None,
            background: true,
            check_interval: 24,
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_24() -> u64 {
    24
}

impl ShellInitConfig {
    /// Compute a hash of the config for stamp comparison.
    ///
    /// Streams the inputs into the hasher directly — no `Vec` / `String`
    /// clones — because this runs on every shell open via `jarvy ensure`.
    pub fn config_hash(&self) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        if let Some(t) = self.tools.as_ref() {
            // Sort indices, not the strings themselves.
            let mut idx: Vec<&str> = t.iter().map(String::as_str).collect();
            idx.sort_unstable();
            for (i, name) in idx.iter().enumerate() {
                if i > 0 {
                    hasher.update(b",");
                }
                hasher.update(name.as_bytes());
            }
            hasher.update(b";");
        }
        if let Some(v) = self.versions.as_ref() {
            let mut keys: Vec<&str> = v.keys().map(String::as_str).collect();
            keys.sort_unstable();
            for k in keys {
                hasher.update(k.as_bytes());
                hasher.update(b"=");
                if let Some(val) = v.get(k) {
                    hasher.update(val.as_bytes());
                }
                hasher.update(b";");
            }
        }
        // Format matches drift::state::hash_string ("sha256:<hex>") so the
        // existing stamp files are not invalidated by this rewrite.
        format!("sha256:{}", hex::encode(hasher.finalize()))
    }

    /// Build (tool_name, version_hint) pairs from config without cloning.
    pub fn tool_tasks(&self) -> Vec<(&str, &str)> {
        let Some(tools) = self.tools.as_ref() else {
            return Vec::new();
        };
        let versions = self.versions.as_ref();
        tools
            .iter()
            .map(|name| {
                let hint = versions
                    .and_then(|v| v.get(name))
                    .map(String::as_str)
                    .unwrap_or("");
                (name.as_str(), hint)
            })
            .collect()
    }
}

/// Stamp file tracking ensure state (~/.jarvy/ensure.stamp)
#[derive(Deserialize, Serialize, Debug)]
pub struct EnsureStamp {
    pub config_hash: String,
    pub last_check: u64,
    pub tools_installed: Vec<String>,
    pub jarvy_version: String,
}

impl EnsureStamp {
    /// Path to the stamp file (canonical resolver in `crate::paths`).
    fn path() -> Option<PathBuf> {
        crate::paths::ensure_stamp().ok()
    }

    /// Load the stamp from disk
    pub fn load() -> Option<Self> {
        let path = Self::path()?;
        let content = fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }

    /// Save the stamp to disk
    pub fn save(&self) -> Result<(), std::io::Error> {
        let path = Self::path().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "no home directory")
        })?;
        let dir = path.parent().unwrap_or_else(|| std::path::Path::new("."));
        fs::create_dir_all(dir)?;
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        // Atomic write: write to unpredictable temp file, then rename
        let tmp = tempfile::NamedTempFile::new_in(dir)?;
        fs::write(tmp.path(), &json)?;
        tmp.persist(&path).map_err(|e| e.error)?;
        Ok(())
    }

    /// Check if the stamp is fresh (config unchanged and within check interval)
    pub fn is_fresh(&self, config_hash: &str, interval_hours: u64) -> bool {
        if self.config_hash != config_hash {
            return false;
        }
        if interval_hours == 0 {
            return false;
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let elapsed_hours = (now.saturating_sub(self.last_check)) / 3600;
        elapsed_hours < interval_hours
    }
}

/// One-line lead pointing users at the log for post-mortem on a
/// silent shell-startup failure. Const so wording drift across the
/// six shell branches is impossible (the strings themselves must
/// stay literal — rc files consume them on future shell starts).
pub const RC_LOG_HINT: &str = "jarvy: ensure failed; see ~/.jarvy/logs/jarvy.log";

/// Generate the RC snippet for a given shell type.
///
/// Besides the `jarvy ensure` startup check, defines `jr` as shorthand for
/// `jarvy run` (the npm-run-style `[commands]` runner) — a function rather
/// than an alias on PowerShell, where aliases can't carry arguments — and
/// `jp`, the agent-profile switcher (PRD-058): bare `jp` lists profiles;
/// `jp <name> [...]` evaluates the `export VAR="path"` lines that
/// `jarvy agents profile use <name> --print-env` prints on stdout into the
/// CURRENT shell (human text goes to stderr, so eval only sees exports).
/// Shells without `eval` parse the lines instead: PowerShell converts them
/// to `env:` writes; nushell builds a record and `load-env`s it.
///
/// The snippet sets `JARVY_ENSURE_INVOCATION=rc_snippet` before calling
/// `jarvy ensure` so `ensure` can tag its telemetry with the invocation
/// source (distinguishes rc-triggered from manual runs).
///
/// Failure surface: with the WarnOnly console default the rc line
/// is otherwise silent — a broken ensure would loop invisibly on
/// every new shell. The `|| echo` (or Windows equivalent) writes
/// one line to stderr on non-zero exit so the user gets a lead.
pub fn generate_rc_snippet(shell: ShellType) -> String {
    match shell {
        // Fish `eval (cmd)` joins the substitution's lines with spaces,
        // mangling multi-export output — `| source` executes each line
        // as-is (fish ≥3.0 ships a bash-compat `export` function), and
        // mirrors the `jarvy shell-init --shell fish | source` loader idiom.
        ShellType::Fish => format!(
            "if command -q jarvy\n  \
             env JARVY_ENSURE_INVOCATION=rc_snippet jarvy ensure --quiet; \
             or echo \"{hint}\" >&2\n  \
             alias jr 'jarvy run'\n  \
             function jp\n    \
             if test (count $argv) -eq 0\n      \
             jarvy agents profile list\n    \
             else\n      \
             env JARVY_JP_INVOCATION=rc_snippet jarvy agents profile use $argv --print-env | source\n    \
             end\n  \
             end\n  \
             function __jarvy_cwd_hint --on-variable PWD\n    \
             env JARVY_CWD_HINT_SESSION=$fish_pid JARVY_CWD_HINT_INVOCATION=rc_snippet jarvy agents profile check-cwd --quiet; or true\n  \
             end\nend",
            hint = RC_LOG_HINT
        ),
        ShellType::PowerShell => format!(
            "if (Get-Command jarvy -ErrorAction SilentlyContinue) {{\n  \
             $env:JARVY_ENSURE_INVOCATION = 'rc_snippet'\n  \
             jarvy ensure --quiet\n  \
             $__jarvy_exit = $LASTEXITCODE\n  \
             Remove-Item Env:JARVY_ENSURE_INVOCATION -ErrorAction SilentlyContinue\n  \
             if ($__jarvy_exit -ne 0) {{ Write-Error \"{hint}\" }}\n  \
             function jr {{ jarvy run @args }}\n  \
             function jp {{\n    \
             if ($args.Count -eq 0) {{ jarvy agents profile list; return }}\n    \
             $env:JARVY_JP_INVOCATION = 'rc_snippet'\n    \
             jarvy agents profile use @args --print-env | ForEach-Object {{\n      \
             if ($_ -match '^export ([A-Za-z_][A-Za-z0-9_]*)=\"(.*)\"$') {{\n        \
             $val = [regex]::Replace($Matches[2], '\\\\(.)', {{ param($x) $x.Groups[1].Value }})\n        \
             Set-Item -Path \"env:$($Matches[1])\" -Value $val\n      \
             }}\n    \
             }}\n    \
             Remove-Item Env:JARVY_JP_INVOCATION -ErrorAction SilentlyContinue\n  \
             }}\n}}",
            hint = RC_LOG_HINT
        ),
        // Nushell has no `eval` — users `source` this from config.nu.
        // The alias and `def` must be top-level: declarations inside an
        // `if` block are scoped to that block in nu. `jp` parses the
        // bash-syntax `export VAR="path"` lines itself (strip prefix,
        // split on first `=`, trim quotes) and applies them via
        // `load-env`; `def --env` makes the mutation stick in the caller.
        ShellType::Nushell => format!(
            "alias jr = jarvy run\n\
             def --env jp [...args] {{\n  \
             if ($args | is-empty) {{\n    \
             jarvy agents profile list\n  \
             }} else {{\n    \
             with-env {{JARVY_JP_INVOCATION: \"rc_snippet\"}} {{ jarvy agents profile use ...$args --print-env }}\n      \
             | lines\n      \
             | where {{|l| $l | str starts-with \"export \" }}\n      \
             | each {{|l|\n          \
             let kv = ($l | str replace \"export \" \"\" | split row -n 2 \"=\")\n          \
             {{name: ($kv | first), value: ($kv | last | str trim -c '\"')}}\n        \
             }}\n      \
             | reduce -f {{}} {{|it, acc| $acc | insert $it.name $it.value }}\n      \
             | load-env\n  \
             }}\n}}\n\
             if (which jarvy | is-not-empty) {{\n  \
             try {{ with-env {{JARVY_ENSURE_INVOCATION: \"rc_snippet\"}} \
             {{ jarvy ensure --quiet }} }} catch \
             {{ print -e \"{hint}\" }}\n}}",
            hint = RC_LOG_HINT
        ),
        _ => format!(
            // Bash, Zsh, Sh
            "if command -v jarvy &> /dev/null; then\n  \
             JARVY_ENSURE_INVOCATION=rc_snippet jarvy ensure --quiet \
             || echo \"{hint}\" >&2\n  \
             alias jr='jarvy run'\n  \
             jp() {{ if [ $# -eq 0 ]; then jarvy agents profile list; \
             else eval \"$(JARVY_JP_INVOCATION=rc_snippet jarvy agents profile use \"$@\" --print-env)\"; fi; }}\n  \
             __jarvy_cwd_hint() {{ JARVY_CWD_HINT_SESSION=\"$$\" JARVY_CWD_HINT_INVOCATION=rc_snippet jarvy agents profile check-cwd --quiet || true; }}\n  \
             if [ -n \"${{ZSH_VERSION:-}}\" ]; then\n    \
             autoload -Uz add-zsh-hook 2>/dev/null && add-zsh-hook chpwd __jarvy_cwd_hint\n  \
             elif [ -n \"${{BASH_VERSION:-}}\" ]; then\n    \
             case \"${{PROMPT_COMMAND:-}}\" in *__jarvy_cwd_hint*) ;; *) PROMPT_COMMAND=\"if [ \\\"$PWD\\\" != \\\"\\${{__JARVY_LAST_PWD:-}}\\\" ]; then __JARVY_LAST_PWD=\\\"$PWD\\\"; __jarvy_cwd_hint; fi${{PROMPT_COMMAND:+; $PROMPT_COMMAND}}\" ;; esac\n  \
             else\n    \
             : # plain sh (dash, BusyBox) has no reliable chpwd hook; __jarvy_cwd_hint is defined but not wired.\n  \
             fi\nfi",
            hint = RC_LOG_HINT
        ),
    }
}

/// Refuse to run ensure when the global config file is writable by anyone
/// other than the owner. This prevents persistence-via-shell-startup attacks
/// where a co-tenant rewrites `~/.jarvy/config.toml` to inject tool installs
/// that fire on every new shell.
#[cfg(unix)]
fn refuse_if_config_is_world_or_group_writable() -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let Some(path) = crate::init::global_config_path() else {
        return Ok(());
    };
    if !path.exists() {
        return Ok(());
    }
    let Ok(meta) = std::fs::metadata(&path) else {
        return Ok(());
    };
    let mode = meta.permissions().mode();
    if mode & 0o022 != 0 {
        return Err(format!(
            "Refusing to run `jarvy ensure`: {} is writable by group/other ({:o}). \
             Run `chmod 600 ~/.jarvy/config.toml` and try again.",
            crate::network::redact_home(&path.display().to_string()),
            mode & 0o777
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn refuse_if_config_is_world_or_group_writable() -> Result<(), String> {
    Ok(())
}

/// Run the ensure check: install missing tools if stamp is stale
pub fn run_ensure(config: &ShellInitConfig, force: bool, quiet: bool) -> Result<(), String> {
    refuse_if_config_is_world_or_group_writable()?;

    let config_hash = config.config_hash();
    let start = std::time::Instant::now();

    // Fast path: check stamp
    if !force
        && let Some(stamp) = EnsureStamp::load()
        && stamp.is_fresh(&config_hash, config.check_interval)
    {
        tracing::debug!(event = "ensure.fast_path", reason = "stamp_fresh");
        return Ok(());
    }

    // Slow path: register tools and install missing ones
    tools::register_all();

    let tasks = config.tool_tasks();
    let mut installed: Vec<String> = Vec::new();
    let mut failed_count: u32 = 0;

    for (name, hint) in &tasks {
        // Check if already installed via `has` (quick PATH check)
        if tools::has(name) && hint.is_empty() {
            installed.push((*name).to_string());
            continue;
        }

        if !quiet {
            eprintln!("jarvy ensure: installing {}...", name);
        }
        // Telemetry runs regardless of --quiet so debug bundles still see the
        // signal even when interactive output is suppressed.
        tracing::info!(
            event = "ensure.tool.start",
            tool = %name,
            hint = %hint,
        );

        match tools::add(name, hint) {
            Ok(_) => {
                if !quiet {
                    eprintln!("jarvy ensure: {} installed", name);
                }
                tracing::info!(event = "ensure.tool.success", tool = %name);
                installed.push((*name).to_string());
            }
            Err(e) => {
                if !quiet {
                    eprintln!("jarvy ensure: {} failed: {}", name, e);
                }
                tracing::warn!(
                    event = "ensure.tool.failed",
                    tool = %name,
                    error = %e,
                );
                failed_count += 1;
            }
        }
    }

    // Write stamp
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let stamp = EnsureStamp {
        config_hash,
        last_check: now,
        tools_installed: installed.clone(),
        jarvy_version: env!("CARGO_PKG_VERSION").to_string(),
    };

    if let Err(e) = stamp.save() {
        if !quiet {
            eprintln!("jarvy ensure: failed to write stamp: {}", e);
        }
        tracing::warn!(event = "ensure.stamp.write_failed", error = %e);
    }

    tracing::info!(
        event = "ensure.run.complete",
        tasks = tasks.len(),
        installed = installed.len(),
        failed = failed_count,
        duration_ms = start.elapsed().as_millis() as u64,
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_init_config_default() {
        let config = ShellInitConfig::default();
        assert!(!config.enabled);
        assert!(config.background);
        assert_eq!(config.check_interval, 24);
    }

    #[test]
    fn test_config_hash_deterministic() {
        let config = ShellInitConfig {
            enabled: true,
            tools: Some(vec!["git".into(), "docker".into()]),
            versions: None,
            background: true,
            check_interval: 24,
        };
        let h1 = config.config_hash();
        let h2 = config.config_hash();
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_config_hash_changes_with_tools() {
        let c1 = ShellInitConfig {
            tools: Some(vec!["git".into()]),
            ..Default::default()
        };
        let c2 = ShellInitConfig {
            tools: Some(vec!["docker".into()]),
            ..Default::default()
        };
        assert_ne!(c1.config_hash(), c2.config_hash());
    }

    #[test]
    fn test_tool_tasks() {
        let config = ShellInitConfig {
            tools: Some(vec!["node".into(), "git".into()]),
            versions: Some(HashMap::from([("node".into(), "20".into())])),
            ..Default::default()
        };
        let tasks = config.tool_tasks();
        assert_eq!(tasks.len(), 2);
        assert!(tasks.contains(&("node", "20")));
        assert!(tasks.contains(&("git", "")));
    }

    #[test]
    fn config_hash_format_is_sha256_prefixed_hex() {
        let config = ShellInitConfig {
            tools: Some(vec!["git".into()]),
            ..Default::default()
        };
        let h = config.config_hash();
        assert!(h.starts_with("sha256:"));
        let hex = &h["sha256:".len()..];
        assert_eq!(hex.len(), 64);
        assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn config_hash_is_independent_of_tool_order() {
        let a = ShellInitConfig {
            tools: Some(vec!["git".into(), "node".into(), "docker".into()]),
            ..Default::default()
        };
        let b = ShellInitConfig {
            tools: Some(vec!["node".into(), "docker".into(), "git".into()]),
            ..Default::default()
        };
        assert_eq!(a.config_hash(), b.config_hash());
    }

    #[test]
    fn test_stamp_freshness() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let stamp = EnsureStamp {
            config_hash: "sha256:abc".into(),
            last_check: now,
            tools_installed: vec![],
            jarvy_version: "0.2".into(),
        };

        // Same hash, within interval
        assert!(stamp.is_fresh("sha256:abc", 24));
        // Different hash
        assert!(!stamp.is_fresh("sha256:def", 24));
        // Interval 0 always stale
        assert!(!stamp.is_fresh("sha256:abc", 0));
    }

    #[test]
    fn test_stamp_expired() {
        let old_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - (25 * 3600); // 25 hours ago

        let stamp = EnsureStamp {
            config_hash: "sha256:abc".into(),
            last_check: old_time,
            tools_installed: vec![],
            jarvy_version: "0.2".into(),
        };

        assert!(!stamp.is_fresh("sha256:abc", 24));
    }

    #[test]
    fn test_generate_rc_snippet_bash() {
        let snippet = generate_rc_snippet(ShellType::Bash);
        assert!(snippet.contains("command -v jarvy"));
        assert!(snippet.contains("jarvy ensure --quiet"));
        assert!(snippet.contains("alias jr='jarvy run'"));
        // jp: bare = list, args = eval the --print-env exports; "$@" so
        // extra flags (e.g. `jp work --agents claude-code`) pass through.
        assert!(snippet.contains("jp() {"));
        assert!(snippet.contains("jarvy agents profile list"));
        assert!(snippet.contains(
            "eval \"$(JARVY_JP_INVOCATION=rc_snippet jarvy agents profile use \"$@\" --print-env)\""
        ));
        // cwd_hint (bash arm): PWD-dedup wrapper wired via PROMPT_COMMAND,
        // check-cwd invoked with --quiet AND the rc-snippet attribution
        // env var. Stderr must NOT be closed (`2>/dev/null` would suppress
        // the stderr_is_tty() TTY probe in check-cwd and kill the nudge).
        assert!(snippet.contains("__JARVY_LAST_PWD"));
        assert!(snippet.contains("check-cwd --quiet"));
        assert!(snippet.contains("JARVY_CWD_HINT_INVOCATION=rc_snippet"));
        assert!(
            !snippet.contains("check-cwd --quiet 2>/dev/null"),
            "bash: check-cwd must NOT redirect stderr — kills TTY probe"
        );
    }

    #[test]
    fn test_generate_rc_snippet_zsh() {
        // Zsh rides the POSIX arm alongside bash/sh but wires cwd_hint
        // through zsh's dedicated chpwd hook (bash wraps PROMPT_COMMAND,
        // plain sh gets nothing). Assert the chpwd wiring plus the
        // rc-snippet attribution env var.
        let snippet = generate_rc_snippet(ShellType::Zsh);
        assert!(snippet.contains("add-zsh-hook chpwd __jarvy_cwd_hint"));
        assert!(snippet.contains("JARVY_CWD_HINT_INVOCATION=rc_snippet"));
    }

    #[test]
    fn test_generate_rc_snippet_fish() {
        let snippet = generate_rc_snippet(ShellType::Fish);
        assert!(snippet.contains("command -q jarvy"));
        assert!(snippet.contains("alias jr 'jarvy run'"));
        assert!(snippet.contains("end"));
        assert!(snippet.contains("function jp"));
        assert!(snippet.contains("jarvy agents profile list"));
        // `| source`, not `eval (...)` — fish eval joins lines with spaces.
        // `env` prefix sets JARVY_JP_INVOCATION for attribution.
        assert!(snippet.contains(
            "env JARVY_JP_INVOCATION=rc_snippet jarvy agents profile use $argv --print-env | source"
        ));
        // cwd_hint (fish arm): --on-variable PWD triggers on cd, and the
        // rc-snippet attribution env var flows to check-cwd.
        assert!(snippet.contains("__jarvy_cwd_hint --on-variable PWD"));
        assert!(snippet.contains("JARVY_CWD_HINT_INVOCATION=rc_snippet"));
        assert!(
            !snippet.contains("check-cwd --quiet 2>/dev/null"),
            "fish: check-cwd must NOT redirect stderr — kills TTY probe"
        );
    }

    #[test]
    fn test_generate_rc_snippet_powershell() {
        let snippet = generate_rc_snippet(ShellType::PowerShell);
        assert!(snippet.contains("Get-Command"));
        assert!(snippet.contains("jarvy ensure --quiet"));
        assert!(snippet.contains("function jr { jarvy run @args }"));
        // jp parses the bash-syntax export lines into $env: writes.
        assert!(snippet.contains("function jp {"));
        assert!(snippet.contains("jarvy agents profile list"));
        assert!(snippet.contains("jarvy agents profile use @args --print-env"));
        // Attribution env var set before the call and cleaned up after.
        assert!(snippet.contains("$env:JARVY_JP_INVOCATION = 'rc_snippet'"));
        assert!(snippet.contains("Remove-Item Env:JARVY_JP_INVOCATION"));
        // Robust parser: regex match on outer quotes, then single-pass
        // backslash unescape via scriptblock — handles `\"`, `\\`, `\$`, `\``.
        assert!(snippet.contains("[regex]::Replace"));
        assert!(snippet.contains("Set-Item -Path \"env:$($Matches[1])\""));
    }

    #[test]
    fn test_generate_rc_snippet_nushell() {
        let snippet = generate_rc_snippet(ShellType::Nushell);
        assert!(snippet.contains("which jarvy | is-not-empty"));
        assert!(snippet.contains("jarvy ensure --quiet"));
        // Alias must be top-level, before the `if` — nu scopes `alias`
        // declared inside a block to that block.
        assert!(snippet.starts_with("alias jr = jarvy run\n"));
        // jp: `def --env` (so load-env mutates the caller) and also
        // top-level — nu scopes `def` inside a block to that block.
        assert!(snippet.contains("def --env jp [...args] {"));
        assert!(snippet.contains("jarvy agents profile list"));
        // with-env sets JARVY_JP_INVOCATION for attribution; the piped
        // output is then parsed into a record and load-env'd.
        assert!(snippet.contains("with-env {JARVY_JP_INVOCATION: \"rc_snippet\"} { jarvy agents profile use ...$args --print-env }"));
        assert!(snippet.contains("load-env"));
        let def_pos = snippet.find("def --env jp").expect("jp def present");
        let if_pos = snippet.find("if (which jarvy").expect("if block present");
        assert!(
            def_pos < if_pos,
            "jp def must precede the if block (top-level)"
        );
    }

    #[test]
    fn jp_present_in_all_six_shell_snippets() {
        // PRD-058: every shell's snippet must define `jp` with the
        // bare-invocation list fallback and the --print-env switch path.
        for shell in [
            ShellType::Bash,
            ShellType::Zsh,
            ShellType::Sh,
            ShellType::Fish,
            ShellType::PowerShell,
            ShellType::Nushell,
        ] {
            let snippet = generate_rc_snippet(shell);
            assert!(
                snippet.contains("jp"),
                "{shell:?}: jp shorthand missing from snippet"
            );
            assert!(
                snippet.contains("jarvy agents profile list"),
                "{shell:?}: bare jp must list profiles"
            );
            assert!(
                snippet.contains("--print-env"),
                "{shell:?}: jp must use the --print-env switch path"
            );
        }
    }

    #[test]
    fn powershell_and_nushell_do_not_wire_cwd_hint() {
        // PowerShell and Nushell don't yet get the per-`cd` profile-drift
        // nudge — their hook wiring (PowerShell `$PWD`-tracking prompt
        // function, nu's `$env.PWD` didn't exist historically) is deferred
        // to a follow-up. Guard against silent regressions here so a new
        // rc-snippet contributor can't accidentally add half the wiring.
        for shell in [ShellType::PowerShell, ShellType::Nushell] {
            let snippet = generate_rc_snippet(shell);
            assert!(
                !snippet.contains("check-cwd"),
                "{shell:?}: cwd_hint wiring is deferred — must not call check-cwd"
            );
        }
    }

    #[test]
    fn jp_bash_zsh_sh_share_posix_form() {
        // Zsh and Sh ride the Bash arm — the POSIX function must be
        // byte-identical across the three (one arm, no drift).
        let bash = generate_rc_snippet(ShellType::Bash);
        assert_eq!(bash, generate_rc_snippet(ShellType::Zsh));
        assert_eq!(bash, generate_rc_snippet(ShellType::Sh));
        // Bare jp lists; with args it evals the exports with "$@" so
        // extra flags pass through.
        assert!(bash.contains(
            "jp() { if [ $# -eq 0 ]; then jarvy agents profile list; \
             else eval \"$(JARVY_JP_INVOCATION=rc_snippet jarvy agents profile use \"$@\" --print-env)\"; fi; }"
        ));
    }
}
