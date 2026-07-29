//! Profile store: snapshot agents into `~/.jarvy/agent-profiles/`,
//! create/delete named profiles, chmod-verify the store dir.
//!
//! Snapshots preserve file permission bits (credentials keep their
//! original modes) and copy symlinks *as links* — a link inside an
//! agent's config dir is never followed, so a hostile link cannot pull
//! outside content into the store.

use std::fs;
use std::path::{Path, PathBuf};

use super::{ProfileError, ProfileRegistry};
use crate::agents::Agent;

/// Whether a snapshot copies the live config dir (env tier — the live
/// dir stays put) or moves it into the store (symlink tier — the live
/// path becomes a symlink afterwards).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotMode {
    Copy,
    Move,
}

/// Create `~/.jarvy/agent-profiles/` and tighten it to 0700, verifying
/// the chmod actually took effect. Profiles may contain live
/// credentials — a silently-ignored chmod (NFS, drvfs, exFAT) would
/// leave them world-readable, so mirror the
/// `discover.jarvy_toml_perms_unsafe` verify-read-back pattern.
pub fn ensure_store_dirs() -> Result<PathBuf, ProfileError> {
    let dir = crate::paths::agent_profiles_dir()?;
    fs::create_dir_all(&dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(e) = fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)) {
            if crate::observability::telemetry_gate::is_enabled() {
                tracing::warn!(
                    event = "agent_profile.perms_unsafe",
                    error = %e,
                    fs_hint = "chmod_failed",
                );
            }
        } else if let Ok(meta) = fs::metadata(&dir) {
            let mode = meta.permissions().mode() & 0o777;
            if mode != 0o700 && crate::observability::telemetry_gate::is_enabled() {
                tracing::warn!(
                    event = "agent_profile.perms_unsafe",
                    mode = format!("{mode:o}"),
                    fs_hint = "chmod_ignored",
                );
            }
        }
    }
    Ok(dir)
}

/// Resolve a profile's directory, validating the name first (profile
/// names map directly onto directory names under the store).
pub fn profile_dir(name: &str) -> Result<PathBuf, ProfileError> {
    crate::paths::validate_component_name(name)
        .map_err(|_| ProfileError::InvalidName(name.to_string()))?;
    Ok(crate::paths::agent_profiles_dir()?.join(name))
}

/// Snapshot `agent.config_dir()` into `<profile>/<slug>/`.
///
/// An absent config dir is a no-op success (`agent_profile.agent_absent`
/// debug breadcrumb) — snapshotting "everything installed" must not
/// fail on agents that aren't. An existing snapshot for the agent is
/// replaced (that's `save` semantics).
#[cfg(test)]
pub fn snapshot_agent(agent: Agent, profile: &str, mode: SnapshotMode) -> Result<(), ProfileError> {
    let store_root = ensure_store_dirs()?;
    snapshot_agent_at(&store_root, agent, profile, mode)
}

/// Inner variant that takes a pre-computed `store_root` so `init_snapshot`
/// doesn't call `ensure_store_dirs()` once per agent.
fn snapshot_agent_at(
    store_root: &Path,
    agent: Agent,
    profile: &str,
    mode: SnapshotMode,
) -> Result<(), ProfileError> {
    let pdir = profile_dir(profile)?;
    let src = agent.config_dir().ok_or(ProfileError::NoHome)?;

    let meta = match fs::symlink_metadata(&src) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            if crate::observability::telemetry_gate::is_enabled() {
                tracing::debug!(event = "agent_profile.agent_absent", agent = agent.slug());
            }
            return Ok(());
        }
        Err(e) => return Err(e.into()),
    };

    // Moving a path that is already jarvy's own symlink into the store
    // would snapshot a pointer, not content — the agent is already
    // managed; switching is `use`'s job.
    if meta.file_type().is_symlink()
        && mode == SnapshotMode::Move
        && link_points_into(&src, store_root)
    {
        return Err(ProfileError::Io(std::io::Error::other(format!(
            "{} is already managed by the profile store; use `jarvy agents profile use` to switch",
            src.display()
        ))));
    }

    crate::paths::ensure_dir_0700_with_event(&pdir, Some("agent_profile.perms_unsafe"))?;
    let dest = pdir.join(agent.slug());
    remove_existing(&dest)?;

    match mode {
        SnapshotMode::Copy => copy_tree(&src, &dest, agent.slug())?,
        SnapshotMode::Move => {
            if fs::rename(&src, &dest).is_err() {
                // Cross-device rename (EXDEV) — delegate to the extracted
                // helper that logs and surfaces cleanup failures clearly.
                move_dir_cross_device(&src, &dest, agent)?;
            }
        }
    }
    Ok(())
}

/// Create a new named profile dir, optionally seeded from an existing
/// profile (recursive copy). Returns the new profile's path.
pub fn create_profile(name: &str, from: Option<&str>) -> Result<PathBuf, ProfileError> {
    ensure_store_dirs()?;
    let pdir = profile_dir(name)?;
    if fs::symlink_metadata(&pdir).is_ok() {
        return Err(ProfileError::ProfileExists(name.to_string()));
    }
    match from {
        Some(src_name) => {
            let src = profile_dir(src_name)?;
            if !src.is_dir() {
                return Err(ProfileError::ProfileNotFound(src_name.to_string()));
            }
            // `src_name` is a profile dir being copied, not an agent dir;
            // use a placeholder slug for the symlink-safety check.
            copy_tree(&src, &pdir, "<profile>")?;
            // copy_tree preserved the source dir's mode; force 0700
            // regardless — the store invariant beats fidelity here.
            crate::paths::ensure_dir_0700_with_event(&pdir, Some("agent_profile.perms_unsafe"))?;
        }
        None => {
            crate::paths::ensure_dir_0700_with_event(&pdir, Some("agent_profile.perms_unsafe"))?;
        }
    }
    Ok(pdir)
}

/// Delete a profile. Refuses (`ActiveProfileDelete`) while any
/// symlink-tier agent's live path still resolves into this profile —
/// deleting under a live symlink would leave the agent's config dir
/// dangling — or while the registry lists it active for one.
///
/// Returns the count of agent snapshot dirs that existed in the deleted
/// profile (zero if the profile dir had no per-agent subdirs).
pub fn delete_profile(name: &str) -> Result<usize, ProfileError> {
    let pdir = profile_dir(name)?;
    if fs::symlink_metadata(&pdir).is_err() {
        return Err(ProfileError::ProfileNotFound(name.to_string()));
    }
    let mut registry = ProfileRegistry::load()?;
    let mut agent_snapshot_count = 0usize;
    for &agent in Agent::ALL {
        // Count existing agent snapshot dirs (cheap metadata check).
        let snap = pdir.join(agent.slug());
        if fs::symlink_metadata(&snap).is_ok() {
            agent_snapshot_count += 1;
        }
        if !agent.is_symlink_tier() {
            continue;
        }
        if let Some(link) = agent.config_dir()
            && link_points_into(&link, &pdir)
        {
            return Err(ProfileError::ActiveProfileDelete(name.to_string()));
        }
        if registry.active.get(agent.slug()).map(String::as_str) == Some(name) {
            return Err(ProfileError::ActiveProfileDelete(name.to_string()));
        }
    }
    fs::remove_dir_all(&pdir)?;

    // Store hygiene: drop stale env-tier active entries and a stale
    // default so `status` doesn't report a profile that no longer exists.
    let before_active = registry.active.len();
    registry.active.retain(|_, v| v != name);
    let default_stale = registry.default_profile.as_deref() == Some(name);
    if default_stale {
        registry.default_profile = None;
    }
    if registry.active.len() != before_active || default_stale {
        registry.save()?;
    }
    Ok(agent_snapshot_count)
}

/// `jarvy agents profile init` core: snapshot every installed agent
/// into `profile`. Env-tier agents are copied (their live dir keeps
/// working untouched); symlink-tier agents are moved into the store
/// and a symlink is created back at the original path. Idempotent —
/// an already-managed symlink-tier agent is skipped. Returns the
/// agents that were snapshotted this run.
///
/// `only` narrows the set. The symlink tier *moves* the live config dir,
/// so narrowing is the way to snapshot the env tier now and defer an
/// agent whose editor is currently open.
pub fn init_snapshot(profile: &str, only: Option<&[Agent]>) -> Result<Vec<Agent>, ProfileError> {
    // Compute store root once; snapshot_agent_at reuses it per agent
    // rather than re-running ensure_store_dirs() inside each call.
    let store_root = ensure_store_dirs()?;
    let pdir = profile_dir(profile)?;
    let mut snapshotted = Vec::new();
    for &agent in Agent::ALL {
        if only.is_some_and(|list| !list.contains(&agent)) {
            continue;
        }
        let Some(src) = agent.config_dir() else {
            continue;
        };
        let Ok(meta) = fs::symlink_metadata(&src) else {
            continue;
        };
        if agent.is_symlink_tier() {
            if meta.file_type().is_symlink() && link_points_into(&src, &store_root) {
                continue; // already managed
            }
            snapshot_agent_at(&store_root, agent, profile, SnapshotMode::Move)?;
            super::switcher::apply_symlink_repoint(agent, &pdir.join(agent.slug()))?;
        } else {
            snapshot_agent_at(&store_root, agent, profile, SnapshotMode::Copy)?;
        }
        snapshotted.push(agent);
    }
    Ok(snapshotted)
}

/// Which profile name the agent's live symlink currently resolves into,
/// or `None` when the agent is not symlink-tier, is unmanaged, or the
/// link target doesn't resolve into the store. This is the single
/// authoritative implementation; `status.rs` calls this instead of
/// duplicating the logic inline.
pub(crate) fn active_profile_from_link(agent: Agent) -> Option<String> {
    if !agent.is_symlink_tier() {
        return None;
    }
    let dir = agent.config_dir()?;
    let store_root = crate::paths::agent_profiles_dir().ok()?;
    // Must be a jarvy-managed link (not a user's own link to somewhere else).
    if !link_points_into(&dir, &store_root) {
        return None;
    }
    let target = fs::read_link(&dir).ok()?;
    let resolved = if target.is_absolute() {
        target
    } else {
        dir.parent()?.join(&target)
    };
    let rel = match resolved.strip_prefix(&store_root) {
        Ok(r) => r.to_path_buf(),
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

/// `true` when `link` is a symlink whose target lands under `root`.
/// Non-recursive read is enough: jarvy only ever creates single-hop
/// links into the store. Compared against both the raw and the
/// canonicalized root (macOS tempdirs: `/var/...` vs `/private/var/...`)
/// so a link written from either spelling is recognized.
pub(crate) fn link_points_into(link: &Path, root: &Path) -> bool {
    let Ok(target) = fs::read_link(link) else {
        return false;
    };
    let resolved = if target.is_absolute() {
        target
    } else {
        match link.parent() {
            Some(p) => p.join(&target),
            None => target,
        }
    };
    if resolved.starts_with(root) {
        return true;
    }
    match (root.canonicalize(), resolved.canonicalize()) {
        (Ok(root_canon), Ok(resolved_canon)) => resolved_canon.starts_with(root_canon),
        _ => false,
    }
}

/// Remove whatever sits at `path` (dir tree, file, or symlink);
/// missing is fine.
fn remove_existing(path: &Path) -> std::io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_dir() => fs::remove_dir_all(path),
        Ok(_) => fs::remove_file(path),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// Fallback for a cross-device rename (EXDEV): copy then remove.
/// `agent` is only used for telemetry — the operation is generic.
fn move_dir_cross_device(src: &Path, dst: &Path, agent: Agent) -> std::io::Result<()> {
    if crate::observability::telemetry_gate::is_enabled() {
        tracing::debug!(
            event = "agent_profile.snapshot_cross_device",
            agent = agent.slug(),
        );
    }
    copy_tree(src, dst, agent.slug())?;
    if let Err(e) = fs::remove_dir_all(src) {
        if crate::observability::telemetry_gate::is_enabled() {
            tracing::warn!(
                event = "agent_profile.snapshot_cross_device",
                agent = agent.slug(),
                error_kind = "cleanup_failed",
                error = %e,
            );
        }
        return Err(e);
    }
    Ok(())
}

/// Recursive copy preserving permission bits; symlinks are recreated
/// as links (never followed) but only when the target is relative AND
/// stays within `snapshot_root` (the original top-level source dir for
/// this agent). Hostile absolute or escaping links are skipped and
/// emitted as `agent_profile.symlink_skipped` events.
///
/// `agent_slug` is passed in for telemetry only.
fn copy_tree(src: &Path, dst: &Path, agent_slug: &str) -> std::io::Result<()> {
    copy_tree_inner(src, dst, src, agent_slug)
}

fn copy_tree_inner(
    src: &Path,
    dst: &Path,
    snapshot_root: &Path,
    agent_slug: &str,
) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    // Use the metadata already available for `src` to set dst perms.
    if let Ok(meta) = fs::metadata(src) {
        copy_perms_from_meta(&meta, dst);
    }
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        // `file_type()` uses symlink_metadata semantics on all platforms
        // (never follows); `metadata()` is fetched separately for perms.
        let ft = entry.file_type()?; // never follows symlinks
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if ft.is_symlink() {
            // Finding B: refuse symlinks that are absolute or that escape
            // the snapshot source root (lexical check, no canonicalize).
            if symlink_is_safe_to_copy(&from, snapshot_root) {
                recreate_symlink(&from, &to)?;
            } else if crate::observability::telemetry_gate::is_enabled() {
                tracing::warn!(
                    event = "agent_profile.symlink_skipped",
                    agent = agent_slug,
                    path = %from.display(),
                    "skipped unsafe symlink during snapshot copy",
                );
            }
        } else if ft.is_dir() {
            copy_tree_inner(&from, &to, snapshot_root, agent_slug)?;
        } else if !ft.is_file() {
            // Sockets / FIFOs / device nodes are live IPC endpoints, not
            // config (e.g. `~/.codex/ipc/ipc.sock`). `fs::copy` fails on
            // them with ENOTSUP, which would abort the whole snapshot.
            if crate::observability::telemetry_gate::is_enabled() {
                tracing::debug!(
                    event = "agent_profile.special_file_skipped",
                    agent = agent_slug,
                    kind = special_file_kind(&ft),
                    "skipped non-regular file during snapshot copy",
                );
            }
        } else {
            fs::copy(&from, &to)?;
            // Use symlink_metadata (via entry.metadata()) for the already-
            // fetched DirEntry stats — avoids a second stat call (D-2).
            if let Ok(meta) = entry.metadata() {
                copy_perms_from_meta(&meta, &to);
            }
        }
    }
    Ok(())
}

/// Bounded telemetry label for a skipped non-regular file. Paths are not
/// emitted — filenames inside an agent config dir are user content.
fn special_file_kind(ft: &std::fs::FileType) -> &'static str {
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileTypeExt;
        if ft.is_socket() {
            return "socket";
        }
        if ft.is_fifo() {
            return "fifo";
        }
        if ft.is_block_device() {
            return "block_device";
        }
        if ft.is_char_device() {
            return "char_device";
        }
    }
    #[cfg(not(unix))]
    let _ = ft;
    "other"
}

/// Return `true` when the symlink at `link` (inside `source_root`) has a
/// relative target that stays within `source_root` — no absolute targets,
/// no `..` escapes above the source root. Purely lexical; never follows.
fn symlink_is_safe_to_copy(link: &Path, source_root: &Path) -> bool {
    let target = match fs::read_link(link) {
        Ok(t) => t,
        Err(_) => return false,
    };
    // Absolute targets always point outside the snapshot source.
    if target.is_absolute() {
        return false;
    }
    // Resolve the relative target against the link's parent dir.
    let parent = match link.parent() {
        Some(p) => p,
        None => return false,
    };
    let resolved = parent.join(&target);
    // Walk the components and track depth; any `..` that would go above
    // `source_root` is a traversal escape.
    let mut depth: isize = 0;
    // Compute the depth of the link parent relative to source_root to
    // know how many `..` steps are allowed.
    let Ok(base_relative) = parent.strip_prefix(source_root) else {
        return false; // link is not under source_root at all
    };
    let base_depth = base_relative.components().count() as isize;
    for comp in target.components() {
        match comp {
            std::path::Component::ParentDir => {
                depth -= 1;
                if depth < -base_depth {
                    return false; // escaped above source_root
                }
            }
            std::path::Component::Normal(_) => depth += 1,
            _ => {}
        }
    }
    // Final sanity: the resolved path must still start with source_root
    // (handles edge cases in the depth arithmetic).
    resolved.starts_with(source_root)
}

#[cfg_attr(not(unix), allow(unused_variables))]
fn copy_perms_from_meta(meta: &fs::Metadata, dst: &Path) {
    #[cfg(unix)]
    {
        let _ = fs::set_permissions(dst, meta.permissions());
    }
}

fn recreate_symlink(from: &Path, to: &Path) -> std::io::Result<()> {
    let target = fs::read_link(from)?;
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&target, to)
    }
    #[cfg(windows)]
    {
        // Windows needs the dir-vs-file distinction up front; probe the
        // resolved source (dangling links default to a file link).
        if fs::metadata(from).map(|m| m.is_dir()).unwrap_or(false) {
            std::os::windows::fs::symlink_dir(&target, to)
        } else {
            std::os::windows::fs::symlink_file(&target, to)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_profiles::test_support::JarvyHomeGuard;

    #[test]
    fn profile_dir_rejects_invalid_names() {
        for bad in ["", "../x", "a/b", ".hidden", "a\x07b"] {
            assert!(
                matches!(profile_dir(bad), Err(ProfileError::InvalidName(_))),
                "`{bad}` should be refused"
            );
        }
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn create_profile_refuses_dup_and_copies_from() {
        let _home = JarvyHomeGuard::new();
        let a = create_profile("a", None).unwrap();
        assert!(a.is_dir());
        assert!(matches!(
            create_profile("a", None),
            Err(ProfileError::ProfileExists(_))
        ));
        fs::write(a.join("marker.txt"), "hi").unwrap();

        let b = create_profile("b", Some("a")).unwrap();
        assert_eq!(fs::read_to_string(b.join("marker.txt")).unwrap(), "hi");

        assert!(matches!(
            create_profile("c", Some("missing")),
            Err(ProfileError::ProfileNotFound(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn snapshot_copy_preserves_credential_mode() {
        use std::os::unix::fs::PermissionsExt;
        let _home = JarvyHomeGuard::new();
        let src = Agent::ClaudeCode.config_dir().unwrap();
        fs::create_dir_all(src.join("nested")).unwrap();
        let cred = src.join(".credentials.json");
        fs::write(&cred, "{\"token\":\"secret\"}").unwrap();
        fs::set_permissions(&cred, fs::Permissions::from_mode(0o600)).unwrap();
        fs::write(src.join("nested").join("settings.json"), "{}").unwrap();

        snapshot_agent(Agent::ClaudeCode, "work", SnapshotMode::Copy).unwrap();

        let snap = crate::paths::agent_profiles_dir()
            .unwrap()
            .join("work")
            .join("claude-code");
        let mode = fs::metadata(snap.join(".credentials.json"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
        assert!(snap.join("nested").join("settings.json").exists());
        // Copy leaves the live dir in place.
        assert!(cred.exists());
    }

    #[cfg(unix)]
    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn init_snapshot_moves_symlink_tier_and_links_back() {
        let _home = JarvyHomeGuard::new();
        let cursor_dir = Agent::Cursor.config_dir().unwrap();
        fs::create_dir_all(&cursor_dir).unwrap();
        fs::write(cursor_dir.join("mcp.json"), "{}").unwrap();

        let done = init_snapshot("default", None).unwrap();
        assert!(done.contains(&Agent::Cursor));

        // Live path is now a symlink into the store...
        let meta = fs::symlink_metadata(&cursor_dir).unwrap();
        assert!(meta.file_type().is_symlink());
        let store_root = crate::paths::agent_profiles_dir().unwrap();
        assert!(link_points_into(&cursor_dir, &store_root));
        // ...and the content is reachable through it.
        assert!(cursor_dir.join("mcp.json").exists());

        // Idempotent: a second init skips the managed agent.
        let again = init_snapshot("default", None).unwrap();
        assert!(!again.contains(&Agent::Cursor));
    }

    #[cfg(unix)]
    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn delete_refused_while_live_symlink_points_in() {
        let _home = JarvyHomeGuard::new();
        let cursor_dir = Agent::Cursor.config_dir().unwrap();
        fs::create_dir_all(&cursor_dir).unwrap();
        init_snapshot("default", None).unwrap();

        assert!(matches!(
            delete_profile("default"),
            Err(ProfileError::ActiveProfileDelete(_))
        ));
        // Still on disk after the refusal.
        assert!(profile_dir("default").unwrap().is_dir());
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn delete_refused_when_registry_lists_symlink_tier_active() {
        let _home = JarvyHomeGuard::new();
        create_profile("p1", None).unwrap();
        let mut active = std::collections::BTreeMap::new();
        active.insert("cursor".to_string(), "p1".to_string());
        let reg = ProfileRegistry {
            active,
            ..Default::default()
        };
        reg.save().unwrap();

        assert!(matches!(
            delete_profile("p1"),
            Err(ProfileError::ActiveProfileDelete(_))
        ));
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn delete_prunes_env_tier_registry_entries() {
        let _home = JarvyHomeGuard::new();
        create_profile("p2", None).unwrap();
        let mut active = std::collections::BTreeMap::new();
        active.insert("claude-code".to_string(), "p2".to_string());
        let reg = ProfileRegistry {
            default_profile: Some("p2".to_string()),
            active,
            ..Default::default()
        };
        reg.save().unwrap();

        delete_profile("p2").unwrap();
        let back = ProfileRegistry::load().unwrap();
        assert!(back.active.is_empty());
        assert!(back.default_profile.is_none());
        assert!(matches!(
            delete_profile("p2"),
            Err(ProfileError::ProfileNotFound(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn snapshot_copies_inner_symlink_as_link() {
        let _home = JarvyHomeGuard::new();
        let src = Agent::ClaudeCode.config_dir().unwrap();
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("real.txt"), "x").unwrap();
        std::os::unix::fs::symlink("real.txt", src.join("alias.txt")).unwrap();

        snapshot_agent(Agent::ClaudeCode, "links", SnapshotMode::Copy).unwrap();
        let snap = profile_dir("links").unwrap().join("claude-code");
        let meta = fs::symlink_metadata(snap.join("alias.txt")).unwrap();
        assert!(meta.file_type().is_symlink());
        assert_eq!(
            fs::read_link(snap.join("alias.txt")).unwrap(),
            PathBuf::from("real.txt")
        );
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn create_profile_refuses_invalid_names() {
        let _home = JarvyHomeGuard::new();
        let long = "a".repeat(65);
        for bad in ["../x", long.as_str(), "a\x07b", "a/b", ".hidden", ""] {
            assert!(
                matches!(create_profile(bad, None), Err(ProfileError::InvalidName(_))),
                "`{}` should be refused by create_profile",
                bad.escape_debug()
            );
        }
        // The store root itself may exist (ensure_store_dirs runs first),
        // but no profile dirs were created for refused names.
        let store = crate::paths::agent_profiles_dir().unwrap();
        if store.is_dir() {
            let entries: Vec<_> = fs::read_dir(&store)
                .unwrap()
                .filter_map(|e| e.ok())
                .collect();
            assert!(entries.is_empty(), "refused names must not create dirs");
        }
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn snapshot_replaces_existing_snapshot() {
        let _home = JarvyHomeGuard::new();
        let src = Agent::ClaudeCode.config_dir().unwrap();
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("old.txt"), "v1").unwrap();
        snapshot_agent(Agent::ClaudeCode, "work", SnapshotMode::Copy).unwrap();

        // Live dir evolves; re-snapshot must be save semantics — the
        // old snapshot is replaced wholesale, not merged.
        fs::remove_file(src.join("old.txt")).unwrap();
        fs::write(src.join("new.txt"), "v2").unwrap();
        snapshot_agent(Agent::ClaudeCode, "work", SnapshotMode::Copy).unwrap();

        let snap = profile_dir("work").unwrap().join("claude-code");
        assert!(!snap.join("old.txt").exists(), "stale file must be gone");
        assert_eq!(fs::read_to_string(snap.join("new.txt")).unwrap(), "v2");
    }

    #[cfg(unix)]
    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn snapshot_move_on_already_managed_symlink_errors() {
        let _home = JarvyHomeGuard::new();
        let store_root = ensure_store_dirs().unwrap();
        let snap = store_root.join("default").join("cursor");
        fs::create_dir_all(&snap).unwrap();
        let live = Agent::Cursor.config_dir().unwrap();
        std::os::unix::fs::symlink(&snap, &live).unwrap();

        let err = snapshot_agent(Agent::Cursor, "other", SnapshotMode::Move).unwrap_err();
        assert!(
            err.to_string().contains("already managed"),
            "expected already-managed refusal, got: {err}"
        );
        // The live symlink survived the refusal.
        assert!(
            fs::symlink_metadata(&live)
                .unwrap()
                .file_type()
                .is_symlink(),
            "live link must be untouched"
        );
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn snapshot_absent_config_dir_is_noop() {
        let _home = JarvyHomeGuard::new();
        // No live dir seeded for codex.
        snapshot_agent(Agent::Codex, "empty", SnapshotMode::Copy).unwrap();
        let dest = profile_dir("empty").unwrap().join("codex");
        assert!(!dest.exists(), "no-op must not create the snapshot dir");
        // Not even the profile dir is materialized by the early return.
        assert!(!profile_dir("empty").unwrap().exists());
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn snapshot_preserves_deep_nesting_and_empty_dirs() {
        let _home = JarvyHomeGuard::new();
        let src = Agent::ClaudeCode.config_dir().unwrap();
        fs::create_dir_all(src.join("a/b/c")).unwrap();
        fs::write(src.join("a/b/c/deep.txt"), "deep").unwrap();
        fs::create_dir_all(src.join("empty")).unwrap();

        snapshot_agent(Agent::ClaudeCode, "deep", SnapshotMode::Copy).unwrap();

        let snap = profile_dir("deep").unwrap().join("claude-code");
        assert_eq!(
            fs::read_to_string(snap.join("a/b/c/deep.txt")).unwrap(),
            "deep"
        );
        assert!(snap.join("empty").is_dir(), "empty dirs must be preserved");
    }

    /// A live agent leaves IPC endpoints in its config dir (codex keeps
    /// `ipc/ipc.sock`). `fs::copy` returns ENOTSUP on those, which used
    /// to abort the whole snapshot mid-copy.
    #[cfg(unix)]
    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn snapshot_skips_sockets_instead_of_failing() {
        let _home = JarvyHomeGuard::new();
        let src = Agent::ClaudeCode.config_dir().unwrap();
        fs::create_dir_all(src.join("ipc")).unwrap();
        fs::write(src.join("settings.json"), "{}").unwrap();
        std::os::unix::net::UnixListener::bind(src.join("ipc/ipc.sock")).unwrap();

        snapshot_agent(Agent::ClaudeCode, "live", SnapshotMode::Copy).unwrap();

        let snap = profile_dir("live").unwrap().join("claude-code");
        assert_eq!(
            fs::read_to_string(snap.join("settings.json")).unwrap(),
            "{}"
        );
        assert!(
            !snap.join("ipc/ipc.sock").exists(),
            "socket must be skipped, not recreated"
        );
        assert!(snap.join("ipc").is_dir(), "its parent dir still snapshots");
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn delete_leaves_other_profiles_intact() {
        let _home = JarvyHomeGuard::new();
        create_profile("keep", None).unwrap();
        create_profile("drop", None).unwrap();
        fs::write(profile_dir("keep").unwrap().join("marker.txt"), "hi").unwrap();

        let count = delete_profile("drop").unwrap();
        assert_eq!(count, 0, "no agent snapshot dirs in an empty profile");

        assert!(!profile_dir("drop").unwrap().exists());
        assert!(profile_dir("keep").unwrap().is_dir());
        assert_eq!(
            fs::read_to_string(profile_dir("keep").unwrap().join("marker.txt")).unwrap(),
            "hi"
        );
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn delete_returns_agent_snapshot_count() {
        let _home = JarvyHomeGuard::new();
        create_profile("p", None).unwrap();
        // Create two agent snapshot dirs manually.
        fs::create_dir_all(profile_dir("p").unwrap().join("claude-code")).unwrap();
        fs::create_dir_all(profile_dir("p").unwrap().join("codex")).unwrap();
        let count = delete_profile("p").unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn init_snapshot_empty_home_is_empty_and_creates_no_agent_dirs() {
        let _home = JarvyHomeGuard::new();
        let done = init_snapshot("default", None).unwrap();
        assert!(done.is_empty());
        // The store root exists (ensure_store_dirs), but no per-agent
        // snapshot dirs were materialized — not even the profile dir.
        assert!(!profile_dir("default").unwrap().exists());
        for &agent in Agent::ALL {
            let live = agent.config_dir().unwrap();
            assert!(!live.exists(), "{} must not be created", agent.slug());
        }
    }

    #[cfg(unix)]
    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn init_snapshot_env_tier_leaves_live_dir_untouched() {
        let _home = JarvyHomeGuard::new();
        let src = Agent::ClaudeCode.config_dir().unwrap();
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("settings.json"), "{\"k\":1}").unwrap();

        let done = init_snapshot("default", None).unwrap();
        assert!(done.contains(&Agent::ClaudeCode));

        // Env tier is Copy: the live path stays a REAL directory (not a
        // symlink) with its content intact.
        let meta = fs::symlink_metadata(&src).unwrap();
        assert!(meta.file_type().is_dir());
        assert!(!meta.file_type().is_symlink());
        assert_eq!(
            fs::read_to_string(src.join("settings.json")).unwrap(),
            "{\"k\":1}"
        );
        // ...and the snapshot exists too.
        assert!(
            profile_dir("default")
                .unwrap()
                .join("claude-code")
                .join("settings.json")
                .exists()
        );
    }

    #[cfg(unix)]
    #[test]
    fn link_points_into_resolves_relative_targets_and_nonlinks() {
        // No JARVY_HOME involvement — pure tempdir paths.
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("store");
        fs::create_dir_all(root.join("work")).unwrap();

        // Relative target: link's parent is tmp, target "store/work".
        let rel_link = tmp.path().join("rel-link");
        std::os::unix::fs::symlink("store/work", &rel_link).unwrap();
        assert!(link_points_into(&rel_link, &root));

        // Relative target escaping the root.
        let away = tmp.path().join("away-link");
        std::os::unix::fs::symlink("elsewhere", &away).unwrap();
        assert!(!link_points_into(&away, &root));

        // Non-symlink path (real dir) → false.
        assert!(!link_points_into(&root, &root));
        // Missing path → false.
        assert!(!link_points_into(&tmp.path().join("ghost"), &root));
    }

    // ── Finding B tests ────────────────────────────────────────────────

    #[test]
    fn symlink_is_safe_absolute_link_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let source_root = tmp.path().join("src");
        fs::create_dir_all(&source_root).unwrap();
        // Absolute symlink inside source_root → refused.
        let link = source_root.join("abs-link");
        #[cfg(unix)]
        std::os::unix::fs::symlink("/etc/passwd", &link).unwrap();
        #[cfg(windows)]
        {
            // On Windows we can't create a symlink in tests; assert false
            // directly since absolute targets are always refused.
            assert!(!symlink_is_safe_to_copy(&link, &source_root));
            return;
        }
        #[cfg(unix)]
        assert!(
            !symlink_is_safe_to_copy(&link, &source_root),
            "absolute symlink must be refused"
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlink_is_safe_escaping_relative_link_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let source_root = tmp.path().join("src");
        fs::create_dir_all(source_root.join("sub")).unwrap();
        // A relative link that escapes above source_root.
        let link = source_root.join("escape-link");
        std::os::unix::fs::symlink("../../outside", &link).unwrap();
        assert!(
            !symlink_is_safe_to_copy(&link, &source_root),
            "escaping symlink must be refused"
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlink_is_safe_in_tree_relative_link_preserved() {
        let tmp = tempfile::tempdir().unwrap();
        let source_root = tmp.path().join("src");
        fs::create_dir_all(&source_root).unwrap();
        fs::write(source_root.join("real.txt"), "x").unwrap();
        // A relative link pointing at a sibling file — in-tree.
        let link = source_root.join("alias.txt");
        std::os::unix::fs::symlink("real.txt", &link).unwrap();
        assert!(
            symlink_is_safe_to_copy(&link, &source_root),
            "in-tree relative symlink must be allowed"
        );
    }

    #[cfg(unix)]
    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn snapshot_skips_absolute_symlink_inside_agent_dir() {
        // Absolute links planted in an agent dir must NOT propagate into
        // the snapshot; relative in-tree links must be preserved.
        let _home = JarvyHomeGuard::new();
        let src = Agent::ClaudeCode.config_dir().unwrap();
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("safe.txt"), "ok").unwrap();
        // In-tree relative link — safe.
        std::os::unix::fs::symlink("safe.txt", src.join("alias.txt")).unwrap();
        // Absolute link escaping the dir — must be skipped.
        std::os::unix::fs::symlink("/etc/passwd", src.join("hostile.txt")).unwrap();

        snapshot_agent(Agent::ClaudeCode, "test", SnapshotMode::Copy).unwrap();
        let snap = profile_dir("test").unwrap().join("claude-code");
        // The absolute hostile link must NOT exist in the snapshot.
        assert!(
            !snap.join("hostile.txt").exists(),
            "absolute symlink must not propagate"
        );
        // The in-tree relative link IS preserved.
        assert!(
            snap.join("alias.txt").exists() || fs::symlink_metadata(snap.join("alias.txt")).is_ok(),
            "in-tree symlink must be preserved"
        );
    }

    // ── Finding C tests ────────────────────────────────────────────────

    /// move_dir_cross_device surfaces cleanup failures: if the dst copy
    /// succeeded but remove_dir_all fails (e.g. read-only sub-dir), the
    /// function returns Err AND both src and dst remain on disk.
    #[cfg(unix)]
    #[test]
    fn move_dir_cross_device_surfaces_cleanup_failure() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src");
        let dst = tmp.path().join("dst");
        fs::create_dir_all(src.join("sub")).unwrap();
        fs::write(src.join("sub").join("file.txt"), "data").unwrap();
        // Make the subdirectory read-only so remove_dir_all fails.
        fs::set_permissions(src.join("sub"), fs::Permissions::from_mode(0o500)).unwrap();

        let result = move_dir_cross_device(&src, &dst, Agent::ClaudeCode);

        // Restore so tempdir cleanup can remove the directory.
        let _ = fs::set_permissions(src.join("sub"), fs::Permissions::from_mode(0o700));

        assert!(result.is_err(), "cleanup failure must surface as Err");
        // Both src and dst still exist (neither was silently lost).
        assert!(src.exists(), "src must still exist after partial move");
        assert!(
            dst.exists(),
            "dst must exist (copy succeeded before remove failed)"
        );
    }
}
