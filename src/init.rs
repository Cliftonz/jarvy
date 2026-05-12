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

    // Create the .jarvy directory if it doesn't exist
    if !jarvy_dir.exists() {
        if let Err(e) = fs::create_dir(&jarvy_dir) {
            eprintln!("Unable to create jarvy config directory: {e}");
            return CliConfig::default();
        }
        // Notice → stderr. Stdout is reserved for command output so
        // callers piping `jarvy <cmd> --format json` get a clean payload
        // on first run (when ~/.jarvy doesn't yet exist). This used to
        // be a `println!` and broke `scripts/gen-docs.sh` in CI runs
        // that hit a virgin $HOME. Documented in
        // docs/release-quirks-jarvy.md.
        eprintln!(
            r"
        Jarvy tool collects telemetry data to help us improve your experience.
        The data collected is anonymized and used solely for analytics purposes.
        If you wish to opt-out of telemetry collection, you can disable it by adding the following line to your configuration file located at ~/.jarvy/config.toml:
        [settings]
        telemetry = false

        Thank you for using Jarvy!
                "
        );

        // Write initial config
        let config = CliConfig {
            settings: Settings::default(),
            telemetry: TelemetryConfig::default(),
            shell_init: None,
            mcp: McpPreferences::default(),
        };
        let toml = toml::to_string(&config).unwrap_or_default();
        let mut file = match fs::File::create(&config_file_path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Unable to create config file: {e}");
                return CliConfig::default();
            }
        };
        if let Err(e) = file.write_all(toml.as_bytes()) {
            eprintln!("Unable to write content to config file: {e}");
            return CliConfig::default();
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
/// `JARVY_HOME` (handled inside `crate::paths::jarvy_home`) overrides
/// the entire `~/.jarvy/` location and is honored ahead of the
/// test-only override.
pub fn global_config_path() -> Option<std::path::PathBuf> {
    if let Ok(custom_home) = std::env::var("JARVY_TEST_HOME") {
        return Some(
            std::path::PathBuf::from(custom_home)
                .join(".jarvy")
                .join("config.toml"),
        );
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

#[cfg(test)]
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
