//! Switching mechanics (PRD-058): env-tier export planning + the
//! symlink-tier atomic re-point.
//!
//! Env-tier switches never touch disk — the caller (CLI / shell-init
//! `jp` wrapper) evals the export lines in the user's shell, so two
//! terminals can run different profiles at once. Symlink-tier switches
//! are global for that agent: `~/.{agent}` is re-pointed into the
//! profile store with a rename-over so no reader ever observes a
//! missing config dir.

use std::fs;
use std::path::{Path, PathBuf};

use super::ProfileError;
use super::store;
use crate::agents::{Agent, ProfileMechanism};

/// One step of a `use` invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SwitchPlan {
    /// Env-tier exports for the calling shell: `(var, profile snapshot
    /// dir)` pairs, e.g. `CLAUDE_CONFIG_DIR=<profile>/claude-code`.
    EnvExports(Vec<(&'static str, PathBuf)>),
    /// Global symlink re-point for one agent.
    SymlinkRepoint { agent: Agent, target: PathBuf },
}

/// Plan a `jarvy agents profile use <profile>` across `agents`.
///
/// Env-tier agents always yield export entries. Symlink-tier agents
/// are included only when `global` — without it the caller prints a
/// notice that a global swap would be required. Every included agent
/// must already have a snapshot in the profile (`AgentNotSnapshotted`).
pub fn plan_use(
    profile: &str,
    agents: &[Agent],
    global: bool,
) -> Result<Vec<SwitchPlan>, ProfileError> {
    let pdir = store::profile_dir(profile)?;
    if !pdir.is_dir() {
        return Err(ProfileError::ProfileNotFound(profile.to_string()));
    }

    let mut env_exports = Vec::new();
    let mut repoints = Vec::new();
    for &agent in agents {
        let snapshot = pdir.join(agent.slug());
        for mechanism in agent.profile_mechanisms() {
            match mechanism {
                ProfileMechanism::Env { var } => {
                    require_snapshot(&snapshot, agent, profile)?;
                    env_exports.push((*var, snapshot.clone()));
                }
                ProfileMechanism::Symlink => {
                    if global {
                        require_snapshot(&snapshot, agent, profile)?;
                        repoints.push(SwitchPlan::SymlinkRepoint {
                            agent,
                            target: snapshot.clone(),
                        });
                    }
                }
            }
        }
    }

    let mut plans = Vec::new();
    if !env_exports.is_empty() {
        plans.push(SwitchPlan::EnvExports(env_exports));
    }
    plans.extend(repoints);
    Ok(plans)
}

fn require_snapshot(snapshot: &Path, agent: Agent, profile: &str) -> Result<(), ProfileError> {
    if snapshot.is_dir() {
        Ok(())
    } else {
        Err(ProfileError::AgentNotSnapshotted {
            agent: agent.slug().to_string(),
            profile: profile.to_string(),
        })
    }
}

/// Re-point `agent.config_dir()` at `target` (a per-agent snapshot dir
/// inside the profile store).
///
/// Safety: the canonicalized target must live under the user's home —
/// a profile store entry that resolves to `/etc` or another user's
/// dir is refused (`EscapesHome`), adapting the
/// `mcp::safety::resolve_within_workspace` posture to a home-rooted
/// boundary. The live path is inspected with `symlink_metadata`
/// (never `metadata`, which would follow the very link we manage): a
/// real directory there means the user reinstalled the agent — refuse
/// with `UnmanagedDir` instead of clobbering local state.
///
/// Telemetry: the CLI handler emits `agent_profile.switched` — this
/// primitive stays silent so `init_snapshot`'s link-back doesn't
/// false-fire a switch event.
pub fn apply_symlink_repoint(agent: Agent, target: &Path) -> Result<(), ProfileError> {
    ensure_under_home(target)?;
    let link = agent.config_dir().ok_or(ProfileError::NoHome)?;

    match fs::symlink_metadata(&link) {
        Ok(meta) if meta.file_type().is_symlink() => {
            // Atomic re-point: build a sibling temp link, rename over.
            let parent = link
                .parent()
                .ok_or_else(|| ProfileError::Io(std::io::Error::other("link has no parent dir")))?;
            let tmp = parent.join(format!(
                ".{}.jarvy-new-{}",
                agent.slug(),
                std::process::id()
            ));
            let _ = remove_link(&tmp); // stale temp from a crashed run
            make_dir_link(&tmp, target)?;
            if fs::rename(&tmp, &link).is_err() {
                // Windows treats a junction as a directory, so
                // rename-over fails there; fall back to remove-old +
                // rename. The non-atomic window is junction-only.
                if let Err(e) = remove_link(&link).and_then(|()| fs::rename(&tmp, &link)) {
                    let _ = remove_link(&tmp);
                    return Err(e.into());
                }
            }
        }
        Ok(_) => {
            if crate::observability::telemetry_gate::is_enabled() {
                tracing::warn!(event = "agent_profile.unmanaged_dir", agent = agent.slug());
            }
            return Err(ProfileError::UnmanagedDir(link));
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            make_dir_link(&link, target)?;
        }
        Err(e) => return Err(e.into()),
    }

    Ok(())
}

/// Canonicalize `target` and require it to sit under the home dir that
/// produced `Agent::config_dir()` (JARVY_HOME honored). The target
/// always exists here (it's a snapshot dir), so full canonicalization
/// covers `..` components and symlink tricks in one step — no manual
/// component walk needed, unlike `resolve_within_workspace` which must
/// tolerate not-yet-existing files.
fn ensure_under_home(target: &Path) -> Result<(), ProfileError> {
    let home = crate::agents::home_dir().ok_or(ProfileError::NoHome)?;
    let home_canon = home.canonicalize()?;
    let target_canon = target.canonicalize()?;
    if target_canon.starts_with(&home_canon) {
        Ok(())
    } else {
        Err(ProfileError::EscapesHome(target.to_path_buf()))
    }
}

#[cfg(unix)]
fn make_dir_link(link: &Path, target: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn make_dir_link(link: &Path, target: &Path) -> std::io::Result<()> {
    match std::os::windows::fs::symlink_dir(target, link) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            // Real symlinks need SeCreateSymbolicLinkPrivilege (admin /
            // Developer Mode); directory junctions don't. mklink is a
            // cmd.exe builtin, so shell out.
            let status = std::process::Command::new("cmd")
                .arg("/C")
                .arg("mklink")
                .arg("/J")
                .arg(link)
                .arg(target)
                .status()?;
            if status.success() {
                Ok(())
            } else {
                Err(std::io::Error::other("mklink /J failed"))
            }
        }
        Err(e) => Err(e),
    }
}

#[cfg(unix)]
fn remove_link(path: &Path) -> std::io::Result<()> {
    fs::remove_file(path)
}

#[cfg(windows)]
fn remove_link(path: &Path) -> std::io::Result<()> {
    // Dir symlinks and junctions delete via remove_dir (removes the
    // link, not the target); fall back for file links.
    fs::remove_dir(path).or_else(|_| fs::remove_file(path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_profiles::store::create_profile;

    fn pin_jarvy_home() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var("JARVY_HOME", tmp.path());
        }
        tmp
    }

    fn unpin_jarvy_home() {
        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("JARVY_HOME");
        }
    }

    /// Create `<profile>/<slug>/` snapshot dirs directly.
    fn seed_snapshot(profile: &str, agent: Agent) -> PathBuf {
        let dir = store::profile_dir(profile).unwrap().join(agent.slug());
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn plan_use_excludes_symlink_tier_without_global() {
        let _home = pin_jarvy_home();
        create_profile("work", None).unwrap();
        for agent in [Agent::ClaudeCode, Agent::Codex, Agent::Cursor] {
            seed_snapshot("work", agent);
        }

        let agents = [Agent::ClaudeCode, Agent::Codex, Agent::Cursor];
        let plans = plan_use("work", &agents, false).unwrap();
        assert_eq!(plans.len(), 1);
        match &plans[0] {
            SwitchPlan::EnvExports(exports) => {
                assert_eq!(exports.len(), 2);
                assert_eq!(exports[0].0, "CLAUDE_CONFIG_DIR");
                assert_eq!(exports[1].0, "CODEX_HOME");
            }
            other => panic!("expected EnvExports, got {other:?}"),
        }

        let plans = plan_use("work", &agents, true).unwrap();
        assert_eq!(plans.len(), 2);
        assert!(matches!(
            plans[1],
            SwitchPlan::SymlinkRepoint {
                agent: Agent::Cursor,
                ..
            }
        ));
        unpin_jarvy_home();
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn plan_use_requires_snapshot_and_profile() {
        let _home = pin_jarvy_home();
        assert!(matches!(
            plan_use("ghost", &[Agent::ClaudeCode], false),
            Err(ProfileError::ProfileNotFound(_))
        ));

        create_profile("thin", None).unwrap();
        assert!(matches!(
            plan_use("thin", &[Agent::ClaudeCode], false),
            Err(ProfileError::AgentNotSnapshotted { .. })
        ));
        unpin_jarvy_home();
    }

    #[cfg(unix)]
    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn repoint_is_atomic_between_two_profiles() {
        let _home = pin_jarvy_home();
        create_profile("a", None).unwrap();
        create_profile("b", None).unwrap();
        let a = seed_snapshot("a", Agent::Cursor);
        let b = seed_snapshot("b", Agent::Cursor);
        let link = Agent::Cursor.config_dir().unwrap();

        // Absent → created directly.
        apply_symlink_repoint(Agent::Cursor, &a).unwrap();
        assert_eq!(fs::read_link(&link).unwrap(), a);

        // Existing link → renamed over, target updated.
        apply_symlink_repoint(Agent::Cursor, &b).unwrap();
        assert_eq!(fs::read_link(&link).unwrap(), b);
        // No temp link left behind.
        let leftovers: Vec<_> = fs::read_dir(link.parent().unwrap())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.contains("jarvy-new"))
            .collect();
        assert!(leftovers.is_empty(), "stale temp links: {leftovers:?}");
        unpin_jarvy_home();
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn repoint_refuses_unmanaged_real_dir() {
        let _home = pin_jarvy_home();
        create_profile("a", None).unwrap();
        let a = seed_snapshot("a", Agent::Cursor);
        let live = Agent::Cursor.config_dir().unwrap();
        fs::create_dir_all(&live).unwrap();

        assert!(matches!(
            apply_symlink_repoint(Agent::Cursor, &a),
            Err(ProfileError::UnmanagedDir(_))
        ));
        // The real dir survived.
        assert!(fs::symlink_metadata(&live).unwrap().file_type().is_dir());
        unpin_jarvy_home();
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn repoint_refuses_target_outside_home() {
        let _home = pin_jarvy_home();
        let outside = tempfile::tempdir().unwrap();
        assert!(matches!(
            apply_symlink_repoint(Agent::Cursor, outside.path()),
            Err(ProfileError::EscapesHome(_))
        ));
        unpin_jarvy_home();
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn plan_use_symlink_only_without_global_is_empty() {
        let _home = pin_jarvy_home();
        create_profile("work", None).unwrap();
        seed_snapshot("work", Agent::Cursor);

        // Only a symlink-tier agent selected, no --global: nothing to
        // do, but NOT an error — the CLI prints a notice instead.
        let plans = plan_use("work", &[Agent::Cursor], false).unwrap();
        assert!(plans.is_empty(), "expected empty plan, got {plans:?}");
        unpin_jarvy_home();
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn plan_use_orders_env_exports_before_repoints_regardless_of_input_order() {
        let _home = pin_jarvy_home();
        create_profile("work", None).unwrap();
        for agent in [Agent::ClaudeCode, Agent::Codex, Agent::Cursor] {
            seed_snapshot("work", agent);
        }

        // Cursor FIRST in the input; the plan must still lead with the
        // combined EnvExports entry — ordering is by mechanism.
        let plans = plan_use(
            "work",
            &[Agent::Cursor, Agent::ClaudeCode, Agent::Codex],
            true,
        )
        .unwrap();
        assert_eq!(plans.len(), 2);
        assert!(matches!(plans[0], SwitchPlan::EnvExports(_)));
        assert!(matches!(
            plans[1],
            SwitchPlan::SymlinkRepoint {
                agent: Agent::Cursor,
                ..
            }
        ));
        unpin_jarvy_home();
    }

    #[cfg(unix)]
    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn repoint_refuses_regular_file_at_live_path() {
        let _home = pin_jarvy_home();
        create_profile("a", None).unwrap();
        let a = seed_snapshot("a", Agent::Cursor);
        let live = Agent::Cursor.config_dir().unwrap();
        fs::create_dir_all(live.parent().unwrap()).unwrap();
        // A regular FILE at ~/.cursor — the `Ok(_)` arm covers any
        // non-symlink, not just directories.
        fs::write(&live, "not a dir").unwrap();

        assert!(matches!(
            apply_symlink_repoint(Agent::Cursor, &a),
            Err(ProfileError::UnmanagedDir(_))
        ));
        // The file survived the refusal.
        assert_eq!(fs::read_to_string(&live).unwrap(), "not a dir");
        unpin_jarvy_home();
    }

    #[cfg(unix)]
    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn repoint_replaces_users_own_foreign_symlink() {
        // Current behavior (asserted deliberately): the symlink arm
        // matches ANY symlink at the live path — including one the user
        // created themselves (e.g. a dotfile-repo link elsewhere in
        // home). It is re-pointed; the old target dir is untouched.
        let _home = pin_jarvy_home();
        create_profile("a", None).unwrap();
        let a = seed_snapshot("a", Agent::Cursor);

        let foreign = crate::agents::home_dir().unwrap().join("dotfiles-cursor");
        fs::create_dir_all(&foreign).unwrap();
        fs::write(foreign.join("keep.txt"), "mine").unwrap();
        let live = Agent::Cursor.config_dir().unwrap();
        std::os::unix::fs::symlink(&foreign, &live).unwrap();

        apply_symlink_repoint(Agent::Cursor, &a).unwrap();
        assert_eq!(fs::read_link(&live).unwrap(), a);
        // The user's original dir (and content) survives untouched.
        assert_eq!(
            fs::read_to_string(foreign.join("keep.txt")).unwrap(),
            "mine"
        );
        unpin_jarvy_home();
    }

    #[cfg(unix)]
    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn repoint_refuses_snapshot_symlinked_outside_home() {
        // Defense-in-depth: a "snapshot" inside the store that is itself
        // a symlink to somewhere outside home must canonicalize out and
        // be refused — EscapesHome catches the symlink trick, not just a
        // literal outside path.
        let _home = pin_jarvy_home();
        create_profile("evil", None).unwrap();
        let outside = tempfile::tempdir().unwrap();
        let snap = store::profile_dir("evil").unwrap().join("cursor");
        std::os::unix::fs::symlink(outside.path(), &snap).unwrap();

        assert!(matches!(
            apply_symlink_repoint(Agent::Cursor, &snap),
            Err(ProfileError::EscapesHome(_))
        ));
        // No live link was created.
        assert!(fs::symlink_metadata(Agent::Cursor.config_dir().unwrap()).is_err());
        unpin_jarvy_home();
    }
}
