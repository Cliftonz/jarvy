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

/// Generate the RC snippet for a given shell type.
///
/// Besides the `jarvy ensure` startup check, defines `jr` as shorthand for
/// `jarvy run` (the npm-run-style `[commands]` runner) — a function rather
/// than an alias on PowerShell, where aliases can't carry arguments.
pub fn generate_rc_snippet(shell: ShellType) -> String {
    // Failure surface: with the WarnOnly console default the rc line
    // is otherwise silent — a broken ensure would loop invisibly on
    // every new shell. The `|| echo` (or Windows equivalent) writes
    // one line to stderr on non-zero exit so the user gets a lead.
    match shell {
        ShellType::Fish => {
            "if command -q jarvy\n  \
             jarvy ensure --quiet; or echo \"jarvy: ensure failed; see ~/.jarvy/logs/jarvy.log\" >&2\n  \
             alias jr 'jarvy run'\nend"
                .to_string()
        }
        ShellType::PowerShell => {
            "if (Get-Command jarvy -ErrorAction SilentlyContinue) {\n  \
             jarvy ensure --quiet\n  \
             if ($LASTEXITCODE -ne 0) { Write-Error \"jarvy: ensure failed; see ~/.jarvy/logs/jarvy.log\" }\n  \
             function jr { jarvy run @args }\n}"
                .to_string()
        }
        // Nushell has no `eval` — users `source` this from config.nu
        // (e.g. `jarvy shell-init --shell nushell | save -f ~/.config/nushell/jarvy.nu`).
        // The alias must be top-level: `alias` inside an `if` block is
        // scoped to that block in nu. Aliasing a missing external is fine
        // at parse time; it only resolves when invoked.
        ShellType::Nushell => {
            "alias jr = jarvy run\n\
             if (which jarvy | is-not-empty) {\n  \
             try { jarvy ensure --quiet } catch { \
             print -e \"jarvy: ensure failed; see ~/.jarvy/logs/jarvy.log\" }\n}"
                .to_string()
        }
        _ => {
            // Bash, Zsh, Sh
            "if command -v jarvy &> /dev/null; then\n  \
             jarvy ensure --quiet || echo \"jarvy: ensure failed; see ~/.jarvy/logs/jarvy.log\" >&2\n  \
             alias jr='jarvy run'\nfi"
                .to_string()
        }
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
    if !force {
        if let Some(stamp) = EnsureStamp::load() {
            if stamp.is_fresh(&config_hash, config.check_interval) {
                tracing::debug!(event = "ensure.fast_path", reason = "stamp_fresh");
                return Ok(());
            }
        }
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
    }

    #[test]
    fn test_generate_rc_snippet_fish() {
        let snippet = generate_rc_snippet(ShellType::Fish);
        assert!(snippet.contains("command -q jarvy"));
        assert!(snippet.contains("alias jr 'jarvy run'"));
        assert!(snippet.contains("end"));
    }

    #[test]
    fn test_generate_rc_snippet_powershell() {
        let snippet = generate_rc_snippet(ShellType::PowerShell);
        assert!(snippet.contains("Get-Command"));
        assert!(snippet.contains("jarvy ensure --quiet"));
        assert!(snippet.contains("function jr { jarvy run @args }"));
    }

    #[test]
    fn test_generate_rc_snippet_nushell() {
        let snippet = generate_rc_snippet(ShellType::Nushell);
        assert!(snippet.contains("which jarvy | is-not-empty"));
        assert!(snippet.contains("jarvy ensure --quiet"));
        // Alias must be top-level, before the `if` — nu scopes `alias`
        // declared inside a block to that block.
        assert!(snippet.starts_with("alias jr = jarvy run\n"));
    }
}
