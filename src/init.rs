use machineid_rs::{Encryption, HWIDComponent, IdBuilder};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use uuid::Uuid;

use crate::shell_init::ShellInitConfig;
use crate::telemetry::TelemetryConfig;

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub(crate) struct CliConfig {
    pub settings: Settings,
    /// Telemetry configuration (OTLP endpoint, signals, etc.)
    #[serde(default)]
    pub telemetry: TelemetryConfig,
    /// Shell init configuration for `jarvy ensure`
    #[serde(default)]
    pub shell_init: Option<ShellInitConfig>,
    /// MCP server preferences
    #[serde(default)]
    pub mcp: McpPreferences,
}

/// MCP preferences stored in ~/.jarvy/config.toml
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct McpPreferences {
    /// Auto-approve tool installations without prompting
    #[serde(default)]
    pub auto_approve_installs: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct Settings {
    /// Legacy telemetry switch (kept for backward compatibility)
    /// Use [telemetry] section for full configuration
    #[serde(default = "default_true")]
    pub telemetry: bool,
    #[serde(default)]
    pub fingerprint: Option<String>,
}

fn default_true() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            telemetry: true,
            fingerprint: get_hwid_fingerprint().or_else(|| Some(Uuid::now_v7().to_string())),
        }
    }
}

/// Lazily-cached parse of `~/.jarvy/config.toml`. The OnceLock is populated
/// the first time `initialize()` is invoked. Mutating writes through
/// `save_global_config` / `modify_global_config` invalidate the cache so a
/// subsequent `initialize()` call sees the new values.
static GLOBAL_CONFIG: std::sync::OnceLock<std::sync::RwLock<Option<CliConfig>>> =
    std::sync::OnceLock::new();

/// Side-channel for the telemetry disclosure trigger. `initialize_from_disk`
/// runs BEFORE `telemetry::init` (the subscriber + OTLP exporter wiring
/// happen in `main.rs` after the config is loaded), so emitting the
/// `telemetry.disclosure_shown` event inline would always drop — the
/// `is_enabled()` gate reads an uninitialized TELEMETRY OnceLock and
/// returns false. Stash the trigger here; `main.rs` drains it after
/// `telemetry::init` completes.
static PENDING_DISCLOSURE: std::sync::OnceLock<&'static str> = std::sync::OnceLock::new();

/// Return and clear any pending telemetry disclosure trigger. Called
/// from `main.rs` after `telemetry::init` so the OTLP layer is ready
/// to ship the event.
pub(crate) fn take_pending_disclosure() -> Option<&'static str> {
    PENDING_DISCLOSURE.get().copied()
}

fn config_cache() -> &'static std::sync::RwLock<Option<CliConfig>> {
    GLOBAL_CONFIG.get_or_init(|| std::sync::RwLock::new(None))
}

/// Drop the cached `CliConfig` so the next `initialize()` call re-reads from
/// disk. Called by save paths to avoid stale reads.
pub(crate) fn invalidate_global_config_cache() {
    if let Ok(mut guard) = config_cache().write() {
        *guard = None;
    }
}

pub(crate) fn initialize() -> CliConfig {
    if let Ok(guard) = config_cache().read() {
        if let Some(cfg) = guard.as_ref() {
            return cfg.clone();
        }
    }
    let fresh = initialize_from_disk();
    if let Ok(mut guard) = config_cache().write() {
        *guard = Some(fresh.clone());
    }
    fresh
}

fn initialize_from_disk() -> CliConfig {
    // Test probe: allow tests to assert initialization ordering without side-effects
    if std::env::var("JARVY_INIT_PROBE").as_deref() == Ok("1") {
        eprintln!("TEST: initialize called");
    }
    // In test mode, avoid any filesystem side effects and just return defaults
    if std::env::var("JARVY_TEST_MODE").as_deref() == Ok("1") {
        return CliConfig::default();
    }

    // Resolve `~/.jarvy/` and `~/.jarvy/config.toml` through the canonical
    // resolver so a future XDG migration / `JARVY_HOME` override is honored.
    let jarvy_dir = match crate::paths::jarvy_home() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to get home directory: {e}");
            return CliConfig::default();
        }
    };
    let config_file_path = jarvy_dir.join("config.toml");

    // Create the .jarvy directory if it doesn't exist. Notice →
    // stderr (stdout stays clean for `--format json` consumers).
    let is_first_run = !jarvy_dir.exists();
    if is_first_run {
        if let Err(e) = fs::create_dir(&jarvy_dir) {
            eprintln!("Unable to create jarvy config directory: {e}");
            return CliConfig::default();
        }
    }

    // Load the current on-disk config content (empty if missing).
    let config_content = fs::read_to_string(&config_file_path).unwrap_or_default();

    // Telemetry is **opt-out** (CLAUDE.md commitment). On the first
    // run we have nothing on disk; on subsequent runs the user may
    // have a config that pre-dates the `[telemetry]` block. In both
    // cases the user has not persisted an explicit decision —
    // `telemetry::user_decided` is the section-aware predicate that
    // returns false. The boxed disclosure must surface, the config
    // must then be rewritten with `enabled = true` so the next run
    // is "decided" and the disclosure doesn't repeat.
    if !crate::telemetry::user_decided(&config_content) {
        let trigger: &'static str = if is_first_run {
            "first_run"
        } else {
            "legacy_upgrade"
        };
        render_telemetry_disclosure();
        // Telemetry isn't initialized yet — stash the trigger for
        // main.rs to emit after `telemetry::init`.
        let _ = PENDING_DISCLOSURE.set(trigger);

        // Build the persisted config: keep any other fields the user
        // already set; only ensure `[telemetry]` has the default
        // (opt-out, enabled = true) so future runs are "decided".
        let mut persisted: CliConfig = toml::from_str(&config_content).unwrap_or_default();
        persisted.telemetry = TelemetryConfig::default();
        if let Ok(toml) = toml::to_string(&persisted) {
            match fs::File::create(&config_file_path) {
                Ok(mut file) => {
                    if let Err(e) = file.write_all(toml.as_bytes()) {
                        eprintln!("Unable to write content to config file: {e}");
                    }
                }
                Err(e) => eprintln!("Unable to create config file: {e}"),
            }
        }
    }

    // Read existing or just-created config.toml
    let config: CliConfig = {
        let config_content = fs::read_to_string(&config_file_path).unwrap_or_default();
        if config_content.trim().is_empty() {
            CliConfig::default()
        } else {
            toml::from_str(&config_content).unwrap_or_default()
        }
    };

    config
}

/// Render the opt-out telemetry disclosure to stderr.
///
/// Triggered by `initialize_from_disk` on every run where the user
/// has no persisted `[telemetry] enabled` decision (first run or a
/// legacy config that pre-dates the `[telemetry]` block). The file
/// is written with `enabled = true` regardless of whether the user
/// reads the banner — this is a disclosure, not a consent prompt.
/// The boxed format is deliberate: an opt-out default is only
/// ethically defensible when the disclosure is unmissable.
fn render_telemetry_disclosure() {
    eprintln!(
        r#"
╭─────────────────────────────────────────────────────────────────╮
│  Jarvy telemetry is currently ENABLED.                          │
│                                                                 │
│  Anonymized usage data (which tools you install, setup          │
│  durations, failure categories) helps prioritize what to fix    │
│  and improve. No file contents, no command output, no           │
│  hostnames, no IPs. Full schema + data-handling policy:         │
│    https://jarvy.dev/telemetry/                                 │
│                                                                 │
│  Forwarder security model (TLS, rate limits, PII scrubbing,     │
│  fan-out to Grafana Cloud):                                     │
│    https://jarvy.dev/operations/telemetry-forwarder/            │
│                                                                 │
│  Opt out (you can opt back in any time):                        │
│    jarvy telemetry disable                                      │
│                                                                 │
│  Or per-invocation:                                             │
│    JARVY_TELEMETRY=0 jarvy <cmd>                                │
╰─────────────────────────────────────────────────────────────────╯
"#
    );
}

/// Save the global config back to ~/.jarvy/config.toml
pub fn save_global_config(config: &CliConfig) -> Result<(), String> {
    let path = global_config_path().ok_or_else(|| "no home directory".to_string())?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("failed to create config dir: {e}"))?;
    }
    let toml =
        toml::to_string_pretty(config).map_err(|e| format!("failed to serialize config: {e}"))?;
    fs::write(&path, toml).map_err(|e| format!("failed to write config: {e}"))?;
    invalidate_global_config_cache();
    Ok(())
}

/// Single source of truth for the global config path: `~/.jarvy/config.toml`.
///
/// Honors `JARVY_TEST_HOME` as a deliberate test override (overrides the
/// home directory; the `.jarvy` segment is appended). `dirs::home_dir()`
/// on Windows uses `SHGetKnownFolderPath` and ignores HOME/USERPROFILE,
/// so env-var-based isolation does not work there without an explicit
/// hook. `JARVY_TEST_HOME` is opt-in and Jarvy-namespaced; production
/// environments will never set it.
///
/// The `JARVY_TEST_HOME` branch is gated behind the `test-bypass` Cargo
/// feature (review item 15) so release binaries can't be redirected at
/// runtime even by a hostile parent env.
///
/// `JARVY_HOME` (handled inside `crate::paths::jarvy_home`) overrides
/// the entire `~/.jarvy/` location and is honored ahead of the
/// test-only override.
pub fn global_config_path() -> Option<std::path::PathBuf> {
    #[cfg(feature = "test-bypass")]
    {
        if let Ok(custom_home) = std::env::var("JARVY_TEST_HOME") {
            return Some(
                std::path::PathBuf::from(custom_home)
                    .join(".jarvy")
                    .join("config.toml"),
            );
        }
    }
    crate::paths::config_toml().ok()
}

/// Load the current global config (returning `Default` if missing/unreadable),
/// hand it to `modify`, then atomically persist the result.
///
/// Use this instead of hand-rolling load → mutate → write in callers.
pub(crate) fn modify_global_config<F>(modify: F) -> Result<(), String>
where
    F: FnOnce(&mut CliConfig),
{
    let path = global_config_path().ok_or_else(|| "no home directory".to_string())?;
    let mut config: CliConfig = if path.exists() {
        let content = fs::read_to_string(&path).unwrap_or_default();
        if content.trim().is_empty() {
            CliConfig::default()
        } else {
            toml::from_str(&content).unwrap_or_default()
        }
    } else {
        CliConfig::default()
    };
    modify(&mut config);
    save_global_config(&config)
}

// Every test here drives the `JARVY_TEST_HOME` redirect in
// `global_config_path`, which is compiled only under the `test-bypass`
// feature (review item 15). Without that feature the env override is inert,
// so the tests would resolve the real `~/.jarvy/config.toml` — failing their
// tempdir assertions and polluting the developer's home. Gate the module on
// the same feature: the unit-test analogue of the `required-features` gate
// used for the integration tests. CI (`--all-features`) and local
// `--features test-bypass` runs still exercise them.
#[cfg(all(test, feature = "test-bypass"))]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Serializes tests that mutate $HOME and the global config file. Cargo
    /// runs unit tests in parallel by default; without serialization these
    /// tests would race on `~/.jarvy/config.toml`.
    static HOME_MUTEX: Mutex<()> = Mutex::new(());

    fn with_isolated_home<F: FnOnce(&std::path::Path)>(f: F) {
        let _guard = HOME_MUTEX.lock().unwrap_or_else(|p| p.into_inner());
        let tmp = tempfile::TempDir::new().expect("tempdir");

        // global_config_path() honors JARVY_TEST_HOME above all else.
        // Setting that single var isolates tests on every platform,
        // including Windows where dirs::home_dir() ignores env vars.
        let prev = std::env::var("JARVY_TEST_HOME").ok();

        // SAFETY: tests are serialized via HOME_MUTEX so set_var/remove_var
        // races are prevented by construction.
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var("JARVY_TEST_HOME", tmp.path());
        }
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            f(tmp.path());
        }));
        #[allow(unsafe_code)]
        unsafe {
            match prev {
                Some(v) => std::env::set_var("JARVY_TEST_HOME", v),
                None => std::env::remove_var("JARVY_TEST_HOME"),
            }
        }
        if let Err(payload) = res {
            std::panic::resume_unwind(payload);
        }
    }

    #[test]
    fn save_global_config_creates_jarvy_dir_when_missing() {
        with_isolated_home(|home| {
            let cfg = CliConfig::default();
            save_global_config(&cfg).expect("save");
            assert!(home.join(".jarvy").join("config.toml").exists());
        });
    }

    #[test]
    fn modify_global_config_updates_existing_field() {
        with_isolated_home(|home| {
            // Seed with a config that has telemetry disabled.
            let mut initial = CliConfig::default();
            initial.telemetry.enabled = false;
            save_global_config(&initial).expect("seed");

            modify_global_config(|cfg| {
                cfg.telemetry.enabled = true;
                cfg.mcp.auto_approve_installs = true;
            })
            .expect("modify");

            let path = home.join(".jarvy").join("config.toml");
            let content = std::fs::read_to_string(path).unwrap();
            let reloaded: CliConfig = toml::from_str(&content).expect("reparse");
            assert!(reloaded.telemetry.enabled);
            assert!(reloaded.mcp.auto_approve_installs);
        });
    }

    #[test]
    fn modify_global_config_creates_when_missing() {
        with_isolated_home(|home| {
            // No config exists yet.
            assert!(!home.join(".jarvy").join("config.toml").exists());
            modify_global_config(|cfg| {
                cfg.mcp.auto_approve_installs = true;
            })
            .expect("create + modify");
            let content = std::fs::read_to_string(home.join(".jarvy").join("config.toml")).unwrap();
            assert!(content.contains("auto_approve_installs"));
        });
    }

    #[test]
    fn modify_global_config_is_roundtrip_safe() {
        with_isolated_home(|_home| {
            modify_global_config(|cfg| {
                cfg.settings.fingerprint = Some("0123abcd".to_string());
                cfg.telemetry.enabled = true;
            })
            .expect("first");
            modify_global_config(|cfg| {
                // Verify previously-written field is read back, not lost.
                assert_eq!(cfg.settings.fingerprint.as_deref(), Some("0123abcd"));
                cfg.settings.fingerprint = None;
            })
            .expect("second");
            modify_global_config(|cfg| {
                assert!(cfg.settings.fingerprint.is_none());
                assert!(cfg.telemetry.enabled);
            })
            .expect("third");
        });
    }
}

fn get_hwid_fingerprint() -> Option<String> {
    let mut builder = IdBuilder::new(Encryption::SHA256);

    // Add components for the fingerprint.
    builder
        .add_component(HWIDComponent::SystemID) // System UUID
        .add_component(HWIDComponent::CPUCores) // CPU core count
        .add_component(HWIDComponent::OSName) // Operating System name
        .add_component(HWIDComponent::DriveSerial); // Main disk serial

    // Build the ID with a custom key.
    const SALT: &str = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15ac1e289f66085";
    // The key should be constant for your application to ensure consistency.
    builder.build(SALT).ok()
}
