//! AI agent profile switcher (PRD-058 v1, storage + switching core).
//!
//! Named profiles snapshot each agent's whole config dir (credentials,
//! settings, skills, MCP registrations) under
//! `~/.jarvy/agent-profiles/<profile>/<agent-slug>/`. `profiles.json`
//! is the jarvy-owned registry: schema-versioned like
//! `~/.jarvy/update-cache.json`, recording the default profile and the
//! active profile per agent slug.
//!
//! Trust boundaries (PRD-058): everything under the store is 0700,
//! profile *contents* are never logged, and profile NAMES never reach
//! telemetry — events carry counts and bounded agent slugs only. The
//! symlink tier refuses targets that resolve outside `$HOME` and never
//! clobbers a real (unmanaged) directory.
//!
//! The CLI surface (`jarvy agents profile ...`) lives in
//! `commands::agents_profile_cmd`; this module is the library core.

pub mod cwd_hint;
pub mod exclude;
pub mod probe;
pub mod status;
pub mod store;
pub mod switcher;

pub use status::{ProfileStatusReport, check_preference, collect_status};
#[cfg(test)]
pub use store::profile_dir;
pub use store::{
    SaveOutcome, create_profile, delete_profile, init_snapshot, resolve_active_profile,
    restore_agent, save_agent,
};
pub use switcher::{SwitchPlan, apply_symlink_repoint, plan_use};

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Registry-file schema version. Bump when the JSON shape changes so
/// older versions of jarvy don't misparse newer files.
pub const REGISTRY_SCHEMA_VERSION: u32 = 1;

/// Filename inside `~/.jarvy/agent-profiles/`.
pub const REGISTRY_FILENAME: &str = "profiles.json";

/// Everything that can go wrong in the profile store / switcher.
#[derive(Debug, thiserror::Error)]
pub enum ProfileError {
    #[error("cannot determine home directory")]
    NoHome,

    #[error("profile store I/O: {0}")]
    Io(#[from] std::io::Error),

    #[error("profile registry JSON: {0}")]
    Json(#[from] serde_json::Error),

    #[error(
        "invalid profile name `{0}`: names are single path components \
         (no separators, `..`, leading dots, or control bytes; max 64 bytes)"
    )]
    InvalidName(String),

    #[error(
        "refusing to touch `{0}`: it is a real directory, not a \
         jarvy-managed symlink — move it aside or re-run \
         `jarvy agents profile init`"
    )]
    UnmanagedDir(PathBuf),

    #[error("profile `{0}` not found")]
    ProfileNotFound(String),

    #[error("profile `{0}` already exists")]
    ProfileExists(String),

    // Must not reference `save`: that verb is v1.1: see PRD-058 Phasing.
    // Pointing at it strands the user on a command that does not exist.
    #[error(
        "agent `{agent}` has no snapshot in profile `{profile}` — seed the \
         profile with `jarvy agents profile create {profile} --from <existing>`, \
         or run `jarvy agents profile init` to snapshot the live config"
    )]
    AgentNotSnapshotted { agent: String, profile: String },

    #[error(
        "profile `{0}` is active for a symlink-tier agent; switch to \
         another profile before deleting"
    )]
    ActiveProfileDelete(String),

    #[error("symlink target `{0}` resolves outside the home directory")]
    EscapesHome(PathBuf),
}

impl From<crate::paths::NoHomeDir> for ProfileError {
    fn from(_: crate::paths::NoHomeDir) -> Self {
        ProfileError::NoHome
    }
}

/// On-disk registry document (`profiles.json`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProfileRegistry {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    /// Profile new shells auto-activate through the rc snippet.
    #[serde(default)]
    pub default_profile: Option<String>,
    /// Agent slug → active profile name.
    #[serde(default)]
    pub active: BTreeMap<String, String>,
}

fn default_schema_version() -> u32 {
    REGISTRY_SCHEMA_VERSION
}

impl Default for ProfileRegistry {
    fn default() -> Self {
        Self {
            schema_version: REGISTRY_SCHEMA_VERSION,
            default_profile: None,
            active: BTreeMap::new(),
        }
    }
}

/// Canonical registry-file path (`~/.jarvy/agent-profiles/profiles.json`).
pub fn registry_path() -> Result<PathBuf, ProfileError> {
    Ok(crate::paths::agent_profiles_dir()?.join(REGISTRY_FILENAME))
}

impl ProfileRegistry {
    /// Load the registry from disk. Missing file → default (empty)
    /// registry; corrupt JSON bubbles up so callers can report it
    /// rather than silently forgetting which profiles are active.
    pub fn load() -> Result<Self, ProfileError> {
        let path = registry_path()?;
        match fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text).map_err(ProfileError::from),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }

    /// Persist the registry atomically via write-to-temp + rename,
    /// chmod 0600 on both sides of the rename (same shape as
    /// `maintenance::cache::save`).
    pub fn save(&self) -> Result<(), ProfileError> {
        let path = registry_path()?;
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        let body = serde_json::to_string_pretty(self)?;
        // Create the tmp with 0600 from the start (avoids umask 0644 window).
        crate::paths::write_0600(&tmp, &body)?;
        fs::rename(&tmp, &path)?;
        crate::paths::apply_secure_mode_0600(&path);
        Ok(())
    }
}

/// Shared test plumbing for the profile modules and the CLI handler's
/// tests: RAII `JARVY_HOME` pinning + snapshot seeding.
#[cfg(test)]
pub(crate) mod test_support {
    use std::path::PathBuf;

    /// Pins `JARVY_HOME` to a fresh tempdir; unsets it on drop so a
    /// panicking test can't leak the override into the next serial test.
    /// The tempdir is held only for its own Drop cleanup.
    pub(crate) struct JarvyHomeGuard {
        _tmp: tempfile::TempDir,
    }

    impl JarvyHomeGuard {
        pub(crate) fn new() -> Self {
            let tmp = tempfile::tempdir().unwrap();
            #[allow(unsafe_code)]
            unsafe {
                std::env::set_var("JARVY_HOME", tmp.path());
            }
            Self { _tmp: tmp }
        }
    }

    impl Drop for JarvyHomeGuard {
        fn drop(&mut self) {
            #[allow(unsafe_code)]
            unsafe {
                std::env::remove_var("JARVY_HOME");
            }
        }
    }

    /// Create `<profile>/<slug>/` snapshot dirs directly in the store.
    pub(crate) fn seed_snapshot(profile: &str, agent: crate::agents::Agent) -> PathBuf {
        let dir = super::store::profile_dir(profile)
            .unwrap()
            .join(agent.slug());
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::JarvyHomeGuard;
    use super::*;

    #[test]
    fn agent_not_snapshotted_suggests_only_shipped_verbs() {
        let msg = ProfileError::AgentNotSnapshotted {
            agent: "claude-code".to_string(),
            profile: "work".to_string(),
        }
        .to_string();
        assert!(
            !msg.contains("profile save"),
            "error points at `save`, a v1.1 verb that does not exist yet: {msg}"
        );
        assert!(
            msg.contains("--from"),
            "error should name the seeding fix: {msg}"
        );
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn registry_missing_file_defaults() {
        let _home = JarvyHomeGuard::new();
        let reg = ProfileRegistry::load().unwrap();
        assert_eq!(reg.schema_version, REGISTRY_SCHEMA_VERSION);
        assert!(reg.default_profile.is_none());
        assert!(reg.active.is_empty());
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn registry_round_trips_and_cleans_tmp() {
        let _home = JarvyHomeGuard::new();
        let mut active = BTreeMap::new();
        active.insert("claude-code".to_string(), "work".to_string());
        let reg = ProfileRegistry {
            default_profile: Some("work".to_string()),
            active,
            ..Default::default()
        };
        reg.save().unwrap();

        let back = ProfileRegistry::load().unwrap();
        assert_eq!(back.schema_version, REGISTRY_SCHEMA_VERSION);
        assert_eq!(back.default_profile.as_deref(), Some("work"));
        assert_eq!(
            back.active.get("claude-code").map(String::as_str),
            Some("work")
        );

        let path = registry_path().unwrap();
        assert!(path.exists());
        assert!(!path.with_extension("json.tmp").exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
    }

    #[test]
    fn registry_schema_version_defaults_when_absent() {
        // `#[serde(default)]` resilience: an empty document still
        // parses, with the current schema version filled in.
        let reg: ProfileRegistry = serde_json::from_str("{}").unwrap();
        assert_eq!(reg.schema_version, REGISTRY_SCHEMA_VERSION);
        assert!(reg.active.is_empty());
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn registry_corrupt_json_errors_not_silent_default() {
        let _home = JarvyHomeGuard::new();
        let path = registry_path().unwrap();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "{ this is not json").unwrap();
        assert!(
            matches!(ProfileRegistry::load(), Err(ProfileError::Json(_))),
            "corrupt registry must surface, not silently reset active profiles"
        );
    }

    #[test]
    fn registry_unknown_fields_parse_forward_compat() {
        // A newer jarvy may add fields; this version must still parse
        // the document (serde ignores unknown fields by default — no
        // `deny_unknown_fields` on ProfileRegistry).
        let doc = r#"{
            "schema_version": 1,
            "default_profile": "work",
            "active": {"claude-code": "work"},
            "future_field": {"nested": [1, 2, 3]}
        }"#;
        let reg: ProfileRegistry = serde_json::from_str(doc).unwrap();
        assert_eq!(reg.default_profile.as_deref(), Some("work"));
        assert_eq!(
            reg.active.get("claude-code").map(String::as_str),
            Some("work")
        );
    }

    #[test]
    fn registry_schema_version_round_trips_non_default() {
        let reg = ProfileRegistry {
            schema_version: 7,
            ..Default::default()
        };
        let json = serde_json::to_string(&reg).unwrap();
        let back: ProfileRegistry = serde_json::from_str(&json).unwrap();
        assert_eq!(back.schema_version, 7, "non-default version must survive");
    }

    /// The registry file must be 0600 after save — it lives inside the
    /// 0700 store but the file-level mode provides defense-in-depth.
    /// Also verifies no .tmp file survives the atomic rename.
    #[cfg(unix)]
    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn registry_save_mode_is_0600() {
        use std::os::unix::fs::PermissionsExt;
        let _home = JarvyHomeGuard::new();
        ProfileRegistry::default().save().unwrap();
        let path = registry_path().unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "registry file must be 0600, got {:o}", mode);
        assert!(
            !path.with_extension("json.tmp").exists(),
            "tmp file must not survive the rename"
        );
    }
}
