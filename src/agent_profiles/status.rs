//! Per-agent profile status report (PRD-058 v1). Drift detection
//! (mtime + file-count heuristic) is phase 2 — v1 reports mechanism,
//! managed state, and the active profile per agent.

use std::fs;
use std::path::Path;

use serde::Serialize;

use super::store::{active_profile_from_link, link_points_into};
use super::{ProfileError, ProfileRegistry, resolve_active_profile};
use crate::agents::Agent;

/// `jarvy agents profile status` payload (rides the PRD-051
/// `--format json` pattern in WP-B).
#[derive(Debug, Clone, Serialize)]
pub struct ProfileStatusReport {
    /// Profile names present in the store, sorted.
    pub profiles: Vec<String>,
    /// Default profile from the registry (new shells auto-activate it).
    pub default_profile: Option<String>,
    /// One entry per known agent, in `Agent::ALL` order.
    pub agents: Vec<AgentProfileStatus>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentProfileStatus {
    /// Agent slug (`claude-code`, `cursor`, ...).
    pub agent: String,
    /// `"env"` or `"symlink"`.
    pub mechanism: String,
    /// Whether switching is supported for this agent (every current
    /// agent is switchable after the follow-up to PRD-058 v1 —
    /// windsurf/cline/continue graduated from storage-only). Kept as an
    /// explicit field so consumers can filter without recomputing.
    pub switchable: bool,
    /// Active profile, when one can be determined.
    pub active_profile: Option<String>,
    /// `"managed"` | `"unmanaged"` | `"not_installed"`.
    pub state: String,
}

/// Collect the full status report.
///
/// Symlink-tier state comes from `symlink_metadata` of the live config
/// dir (symlink → managed, real dir → unmanaged, absent →
/// not_installed) with the active profile read off the link target
/// itself — the filesystem is the source of truth there. Env-tier
/// state comes from the registry (`profiles.json`), since env-var
/// activation leaves no on-disk marker at the config dir.
pub fn collect_status() -> Result<ProfileStatusReport, ProfileError> {
    let registry = ProfileRegistry::load()?;
    let store_root = crate::paths::agent_profiles_dir()?;

    let mut profiles = Vec::new();
    if store_root.is_dir() {
        for entry in fs::read_dir(&store_root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue; // profiles.json, temp files
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            // Skip anything that isn't a valid profile name (`.git`
            // from the v1.1 sync repo, editor droppings, ...).
            if crate::paths::validate_component_name(&name).is_ok() {
                profiles.push(name);
            }
        }
    }
    profiles.sort();

    let mut agents = Vec::with_capacity(Agent::COUNT);
    for &agent in Agent::ALL {
        agents.push(agent_status(agent, &registry, &store_root));
    }

    Ok(ProfileStatusReport {
        profiles,
        default_profile: registry.default_profile,
        agents,
    })
}

fn agent_status(agent: Agent, registry: &ProfileRegistry, store_root: &Path) -> AgentProfileStatus {
    let symlink_tier = agent.is_symlink_tier();
    let slug = agent.slug();

    let (state, active_profile) = match agent.config_dir() {
        None => ("not_installed", None),
        Some(dir) => match fs::symlink_metadata(&dir) {
            Err(_) => ("not_installed", None),
            Ok(_) if !symlink_tier => {
                let active = registry.active.get(slug).cloned();
                let state = if active.is_some() {
                    "managed"
                } else {
                    "unmanaged"
                };
                (state, active)
            }
            Ok(meta) if meta.file_type().is_symlink() => {
                if link_points_into(&dir, store_root) {
                    // Use the single shared implementation from store.
                    let active = active_profile_from_link(agent)
                        .or_else(|| registry.active.get(slug).cloned());
                    ("managed", active)
                } else {
                    // Symlink, but the user's own (e.g. dotfile repo) —
                    // not jarvy's to re-point.
                    ("unmanaged", None)
                }
            }
            Ok(_) => ("unmanaged", None),
        },
    };

    AgentProfileStatus {
        agent: slug.to_string(),
        mechanism: if symlink_tier { "symlink" } else { "env" }.to_string(),
        switchable: agent.profile_switchable(),
        active_profile,
        state: state.to_string(),
    }
}

/// Outcome of comparing the live profile against `[agents.profiles] prefer`.
#[derive(Debug, Clone, Default)]
pub struct PreferenceCheck {
    /// Slugs of switchable agents sitting on a profile other than the
    /// preferred one.
    pub mismatched: Vec<&'static str>,
    /// Slugs of installed switchable agents jarvy isn't managing at all,
    /// so no profile can be attributed to them.
    pub unmanaged: Vec<&'static str>,
}

impl PreferenceCheck {
    /// True when nothing contradicts the preference. An all-unmanaged
    /// machine matches: the repo asked for a profile and the user simply
    /// isn't using profiles, which is a hint about `unmanaged`, not a
    /// mismatch.
    pub fn matched(&self) -> bool {
        self.mismatched.is_empty()
    }
}

/// Compare each switchable agent's live profile against `prefer`.
///
/// Env-tier agents are read from their config-dir env var rather than
/// the registry: `profiles.json` records the last *global* switch, but
/// `CLAUDE_CONFIG_DIR` is what this terminal is actually on, and the
/// whole point of the env tier is that two shells can differ. Falling
/// back to the registry when the var is unset keeps a plain
/// `jarvy setup` in a fresh shell honest.
///
/// Read-only by construction — `prefer` is advisory and jarvy never
/// auto-switches, because switching moves credentials.
pub fn check_preference(prefer: &str) -> Result<PreferenceCheck, ProfileError> {
    let registry = ProfileRegistry::load()?;
    let store_root = crate::paths::agent_profiles_dir()?;
    let mut check = PreferenceCheck::default();

    for &agent in Agent::ALL {
        if !agent.profile_switchable() || !agent.is_installed() {
            continue;
        }
        // Single shared helper: same "env var → registry → symlink"
        // ordering as `save_action`. Previously each callsite spelled
        // its own walk (mechanism.first vs full loop) and the two
        // would drift the day an agent had multiple mechanisms.
        let active = resolve_active_profile(agent, &registry, &store_root);
        match active {
            Some(name) if name == prefer => {}
            Some(_) => check.mismatched.push(agent.slug()),
            None => check.unmanaged.push(agent.slug()),
        }
    }
    Ok(check)
}

/// Extract the profile name from a link target
/// (`<store>/<profile>/<slug>`): the first component under the store
/// root, tried against both the raw and canonicalized root spellings
/// (macOS `/var` vs `/private/var`). Only called from unit tests now —
/// live code uses `store::active_profile_from_link`.
#[cfg(test)]
fn profile_name_from_link(store_root: &Path, link: &Path, target: &Path) -> Option<String> {
    let resolved = if target.is_absolute() {
        target.to_path_buf()
    } else {
        link.parent()?.join(target)
    };
    let rel = match resolved.strip_prefix(store_root) {
        Ok(rel) => rel.to_path_buf(),
        Err(_) => {
            let root_canon = store_root.canonicalize().ok()?;
            let resolved_canon = resolved.canonicalize().ok()?;
            resolved_canon.strip_prefix(&root_canon).ok()?.to_path_buf()
        }
    };
    rel.components()
        .next()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_profiles::store::create_profile;
    use crate::agent_profiles::test_support::JarvyHomeGuard;

    fn status_for<'a>(report: &'a ProfileStatusReport, slug: &str) -> &'a AgentProfileStatus {
        report
            .agents
            .iter()
            .find(|a| a.agent == slug)
            .unwrap_or_else(|| panic!("no status entry for {slug}"))
    }

    /// Unsets the config-dir var on drop so a panicking test can't leak
    /// it into the next serial test — same shape as `JarvyHomeGuard`.
    struct ConfigDirEnvGuard(&'static str);

    impl ConfigDirEnvGuard {
        fn set(var: &'static str, value: &std::path::Path) -> Self {
            #[allow(unsafe_code)]
            unsafe {
                std::env::set_var(var, value);
            }
            Self(var)
        }
    }

    impl Drop for ConfigDirEnvGuard {
        fn drop(&mut self) {
            #[allow(unsafe_code)]
            unsafe {
                std::env::remove_var(self.0);
            }
        }
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn empty_store_reports_defaults() {
        let _home = JarvyHomeGuard::new();
        let report = collect_status().unwrap();
        assert!(report.profiles.is_empty());
        assert!(report.default_profile.is_none());
        assert_eq!(report.agents.len(), Agent::COUNT);
        assert_eq!(status_for(&report, "claude-code").state, "not_installed");
        assert_eq!(status_for(&report, "claude-code").mechanism, "env");
        assert_eq!(status_for(&report, "cursor").mechanism, "symlink");
        // windsurf graduated to switchable — the report reflects that.
        assert!(status_for(&report, "windsurf").switchable);
    }

    #[cfg(unix)]
    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn full_report_covers_all_states() {
        let _home = JarvyHomeGuard::new();
        // Store: two profiles + the registry file (must be skipped in
        // the profile listing).
        create_profile("work", None).unwrap();
        create_profile("play", None).unwrap();
        let mut active = std::collections::BTreeMap::new();
        active.insert("claude-code".to_string(), "work".to_string());
        let reg = ProfileRegistry {
            default_profile: Some("work".to_string()),
            active,
            ..Default::default()
        };
        reg.save().unwrap();

        // claude-code (env tier): live dir + registry entry → managed.
        fs::create_dir_all(Agent::ClaudeCode.config_dir().unwrap()).unwrap();
        // codex (env tier): live dir, no registry entry → unmanaged.
        fs::create_dir_all(Agent::Codex.config_dir().unwrap()).unwrap();
        // cursor (symlink tier): jarvy-managed link into `work`.
        let snap = crate::paths::agent_profiles_dir()
            .unwrap()
            .join("work")
            .join("cursor");
        fs::create_dir_all(&snap).unwrap();
        std::os::unix::fs::symlink(&snap, Agent::Cursor.config_dir().unwrap()).unwrap();
        // continue (symlink tier): real dir → unmanaged.
        fs::create_dir_all(Agent::Continue.config_dir().unwrap()).unwrap();
        // windsurf / cline: absent → not_installed.

        let report = collect_status().unwrap();
        assert_eq!(report.profiles, vec!["play", "work"]);
        assert_eq!(report.default_profile.as_deref(), Some("work"));

        let claude = status_for(&report, "claude-code");
        assert_eq!(claude.state, "managed");
        assert_eq!(claude.active_profile.as_deref(), Some("work"));

        let codex = status_for(&report, "codex");
        assert_eq!(codex.state, "unmanaged");
        assert!(codex.active_profile.is_none());

        let cursor = status_for(&report, "cursor");
        assert_eq!(cursor.state, "managed");
        assert_eq!(cursor.active_profile.as_deref(), Some("work"));

        assert_eq!(status_for(&report, "continue").state, "unmanaged");
        assert_eq!(status_for(&report, "windsurf").state, "not_installed");
        assert_eq!(status_for(&report, "cline").state, "not_installed");
    }

    /// `profiles.json` records the last *global* switch, but the env
    /// tier's whole point is that two terminals can differ — so the
    /// hint has to read `CLAUDE_CONFIG_DIR`, not the registry, or a
    /// user who switched in this shell gets nagged about the profile
    /// they just left.
    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn preference_reads_env_tier_from_env_var_not_registry() {
        let _home = JarvyHomeGuard::new();
        create_profile("work", None).unwrap();
        create_profile("play", None).unwrap();
        let mut active = std::collections::BTreeMap::new();
        active.insert("claude-code".to_string(), "work".to_string());
        ProfileRegistry {
            active,
            ..Default::default()
        }
        .save()
        .unwrap();
        fs::create_dir_all(Agent::ClaudeCode.config_dir().unwrap()).unwrap();

        // Registry says "work"; this shell is on "play".
        let play = crate::paths::agent_profiles_dir()
            .unwrap()
            .join("play")
            .join("claude-code");
        fs::create_dir_all(&play).unwrap();
        let config_dir = ConfigDirEnvGuard::set("CLAUDE_CONFIG_DIR", &play);

        let check = check_preference("play").unwrap();
        assert!(check.matched(), "env var must win over the registry");

        let check = check_preference("work").unwrap();
        assert_eq!(check.mismatched, vec!["claude-code"]);

        drop(config_dir);
        // With the var gone, the registry is the fallback again.
        assert!(check_preference("work").unwrap().matched());
    }

    /// An installed agent jarvy isn't managing is not a *mismatch* —
    /// the repo asked for a profile and the user simply isn't using
    /// profiles. Conflating the two would nag every non-adopter.
    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn preference_separates_unmanaged_from_mismatched() {
        let _home = JarvyHomeGuard::new();
        create_profile("work", None).unwrap();
        fs::create_dir_all(Agent::Codex.config_dir().unwrap()).unwrap();

        let check = check_preference("work").unwrap();
        assert!(check.matched());
        assert_eq!(check.unmanaged, vec!["codex"]);
        assert!(check.mismatched.is_empty());
    }

    /// Uninstalled agents never appear in the hint. windsurf/cline/
    /// continue used to be filtered as "storage-only, not switchable
    /// until v1.1" — after their graduation to switchable they DO
    /// surface, and an installed-but-unmanaged symlink-tier agent
    /// lands in `unmanaged` (the hint about `--global` still applies).
    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn preference_ignores_uninstalled_agents() {
        let _home = JarvyHomeGuard::new();
        create_profile("work", None).unwrap();
        // No agent config dirs on disk → nothing to compare, matched.
        let check = check_preference("work").unwrap();
        assert!(check.matched());
        assert!(check.unmanaged.is_empty() && check.mismatched.is_empty());
    }

    /// Once windsurf/cline/continue are switchable, installing them but
    /// not managing them (no symlink into the store) must surface in
    /// `unmanaged` — the same hint claude-code/codex/cursor already got.
    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn preference_surfaces_unmanaged_windsurf_continue() {
        let _home = JarvyHomeGuard::new();
        create_profile("work", None).unwrap();
        fs::create_dir_all(Agent::Windsurf.config_dir().unwrap()).unwrap();
        fs::create_dir_all(Agent::Continue.config_dir().unwrap()).unwrap();

        let check = check_preference("work").unwrap();
        assert!(check.matched(), "unmanaged is not a mismatch");
        // Both are installed, both are unmanaged → both surface.
        assert!(check.unmanaged.contains(&"windsurf"));
        assert!(check.unmanaged.contains(&"continue"));
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn status_skips_stray_files_and_dot_dirs_in_store() {
        let _home = JarvyHomeGuard::new();
        create_profile("work", None).unwrap();
        let store = crate::paths::agent_profiles_dir().unwrap();
        // A stray regular file (temp file, editor dropping) is skipped
        // by the file-type check...
        fs::write(store.join("notes.txt"), "scratch").unwrap();
        // ...and a `.git` DIRECTORY (v1.1 sync repo) is skipped by the
        // component-name validation (leading dot).
        fs::create_dir_all(store.join(".git")).unwrap();

        let report = collect_status().unwrap();
        assert_eq!(report.profiles, vec!["work"]);
    }

    #[test]
    fn profile_name_from_link_resolves_relative_target() {
        // Pure path math — no filesystem, no JARVY_HOME.
        let store_root = Path::new("/home/u/.jarvy/agent-profiles");
        let link = Path::new("/home/u/.cursor");
        // Relative target resolved against the link's parent (/home/u).
        let name = profile_name_from_link(
            store_root,
            link,
            Path::new(".jarvy/agent-profiles/work/cursor"),
        );
        assert_eq!(name.as_deref(), Some("work"));

        // Absolute target still works.
        let name = profile_name_from_link(
            store_root,
            link,
            Path::new("/home/u/.jarvy/agent-profiles/play/cursor"),
        );
        assert_eq!(name.as_deref(), Some("play"));

        // Target outside the store root (nonexistent, so the
        // canonicalize fallback also fails) → None.
        let name = profile_name_from_link(store_root, link, Path::new("/somewhere/else"));
        assert!(name.is_none());
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn env_tier_active_reflects_registry_even_for_missing_profile() {
        // Documents current behavior: env-tier `active_profile` is read
        // straight off the registry. `delete_profile` prunes entries,
        // but a manual registry edit (or out-of-band dir removal) can
        // name a profile that no longer exists on disk — status reports
        // it as active/managed anyway; no cross-check against the store.
        let _home = JarvyHomeGuard::new();
        let mut active = std::collections::BTreeMap::new();
        active.insert("claude-code".to_string(), "ghost".to_string());
        let reg = ProfileRegistry {
            active,
            ..Default::default()
        };
        reg.save().unwrap();
        fs::create_dir_all(Agent::ClaudeCode.config_dir().unwrap()).unwrap();

        let report = collect_status().unwrap();
        assert!(report.profiles.is_empty(), "no profile dirs on disk");
        let claude = status_for(&report, "claude-code");
        assert_eq!(claude.state, "managed");
        assert_eq!(claude.active_profile.as_deref(), Some("ghost"));
    }
}
