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

/// What a recursive copy is allowed to leave behind.
///
/// This exists because the two callers have opposite safety properties
/// and the difference used to live only in a comment. `copy_tree` is
/// both the snapshot copier *and* the stand-in for `fs::rename` when it
/// fails with EXDEV — and that second caller deletes the source
/// afterwards. Any path skipped on that path is destroyed, not merely
/// left behind, so the distinction is encoded in the type instead of
/// being re-derived correctly at every skip site.
#[derive(Debug, Clone, Copy)]
pub(crate) enum CopyPolicy {
    /// The source stays on disk and authoritative. Non-identity paths
    /// (`super::exclude`) are dropped and hostile symlinks refused —
    /// everything skipped is still reachable where it came from.
    Snapshot(&'static [&'static str]),
    /// The source is removed once the copy lands. Nothing may be
    /// dropped: no exclusions, and symlinks are recreated verbatim
    /// rather than vetted, because "refusing" a link here would delete
    /// the user's own data.
    Relocate,
}

/// Per-snapshot counters, rolled up into one `agent_profile.snapshot_completed`
/// event. Individual skips are debug-level and high-volume; the aggregate is
/// what makes a *miss* visible — a denylist that stops matching shows up as
/// `subtrees_excluded` collapsing and `bytes` jumping, which per-path events
/// cannot reveal because they simply stop being emitted.
///
/// `subtrees_excluded` counts *matches*, not files: the walk skips an
/// excluded directory whole rather than descending it, so one match can
/// stand for a gigabyte.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct SnapshotStats {
    pub files_copied: u64,
    pub bytes: u64,
    pub subtrees_excluded: u64,
    pub symlinks_skipped: u64,
    pub special_skipped: u64,
}

impl SnapshotStats {
    fn merge(&mut self, other: SnapshotStats) {
        self.files_copied += other.files_copied;
        self.bytes += other.bytes;
        self.subtrees_excluded += other.subtrees_excluded;
        self.symlinks_skipped += other.symlinks_skipped;
        self.special_skipped += other.special_skipped;
    }
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

/// Outcome of `save_agent`: env-tier agents copy their live dir over
/// the snapshot; symlink-tier agents are live-in-profile already
/// (their `~/.slug` is a jarvy-managed symlink into the store) so a
/// save is a no-op. When a symlink-tier agent's live path isn't a
/// jarvy-managed link, the agent is unmanaged — save returns
/// `ProfileError::UnmanagedDir` rather than clobbering the user's config.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveOutcome {
    /// Env-tier snapshot refreshed from the live config dir.
    EnvCopied,
    /// Symlink-tier agent: edits are already persisted through the live
    /// symlink; there is nothing to save.
    SymlinkNoOp,
}

/// Refresh a single agent's snapshot in `profile` from its live config
/// dir. Env-tier agents copy over the existing snapshot (same
/// replace-semantics as `snapshot_agent`); symlink-tier agents are
/// already live-in-profile and return `SymlinkNoOp` (or
/// `ProfileError::UnmanagedDir` when the live symlink is missing /
/// detached — save refuses rather than clobbering the user's config).
///
/// The profile must exist first — `save` deliberately does NOT create
/// profiles; the caller composes `create_profile` when that's wanted.
pub fn save_agent(agent: Agent, profile: &str) -> Result<SaveOutcome, ProfileError> {
    // Validate the profile exists so a typo doesn't silently create
    // a new store dir on the next snapshot call.
    let pdir = profile_dir(profile)?;
    if fs::symlink_metadata(&pdir).is_err() {
        return Err(ProfileError::ProfileNotFound(profile.to_string()));
    }

    if agent.is_symlink_tier() {
        // A symlink-tier agent that jarvy manages already writes into
        // its profile snapshot through the live symlink, so saving is a
        // no-op WHEN the live link points at THIS profile. If the link
        // points at a different profile, the caller asked us to
        // snapshot content that lives elsewhere on disk — the store
        // stays authoritative for symlink tier, so the answer is still
        // no-op (any "copy from live" would be a self-copy through the
        // symlink into whichever profile the link happens to target).
        // A real dir or foreign link means the agent is unmanaged —
        // refuse rather than clobber a live editor.
        let Some(live) = agent.config_dir() else {
            return Err(ProfileError::NoHome);
        };
        let store_root = ensure_store_dirs()?;
        return match fs::symlink_metadata(&live) {
            Ok(meta) if meta.file_type().is_symlink() && link_points_into(&live, &store_root) => {
                Ok(SaveOutcome::SymlinkNoOp)
            }
            Ok(_) | Err(_) => Err(ProfileError::UnmanagedDir(live)),
        };
    }

    // Env tier: copy the live dir over the snapshot.
    let store_root = ensure_store_dirs()?;
    snapshot_agent_at(&store_root, agent, profile, SnapshotMode::Copy)?;
    Ok(SaveOutcome::EnvCopied)
}

/// Restore a single agent's live config dir from its `<profile>/<slug>/`
/// snapshot. Env-tier agents get the snapshot copied over their
/// default config dir (NOT the env-var-redirected one — a fresh shell
/// with no env vars reads there). Symlink-tier agents re-point the
/// live symlink to the snapshot via `apply_symlink_repoint`.
///
/// The snapshot must exist — a missing per-agent snapshot returns
/// `AgentNotSnapshotted`.
pub fn restore_agent(agent: Agent, profile: &str) -> Result<(), ProfileError> {
    let snapshot = profile_dir(profile)?.join(agent.slug());
    if fs::symlink_metadata(&snapshot).is_err() {
        return Err(ProfileError::AgentNotSnapshotted {
            agent: agent.slug().to_string(),
            profile: profile.to_string(),
        });
    }

    if agent.is_symlink_tier() {
        // Reuse the same primitive `use --global` uses — it handles the
        // atomic rename, EscapesHome canonicalization, and the
        // UnmanagedDir refusal.
        return super::switcher::apply_symlink_repoint(agent, &snapshot);
    }

    // Env tier: blow away the live dir and copy the snapshot verbatim.
    // The snapshot was already filtered when it landed here, so the
    // denylist doesn't apply on the way out (empty exclusion list).
    let live = agent.config_dir().ok_or(ProfileError::NoHome)?;
    // A live symlink at an env-tier path is by definition user-owned —
    // jarvy doesn't create symlinks for env-tier agents. Refuse to
    // overwrite what the user's dotfile setup put there. The `--force`
    // flag on the restore CLI does NOT bypass this: restoring OVER a
    // user's dotfile link is destructive enough to always require an
    // explicit `rm ~/.claude` first. Reuse `UnmanagedDir` for parity
    // with `apply_symlink_repoint`'s refusal at the symlink-tier path.
    if let Ok(meta) = fs::symlink_metadata(&live)
        && meta.file_type().is_symlink()
    {
        return Err(ProfileError::UnmanagedDir(live));
    }
    remove_existing(&live)?;
    if let Some(parent) = live.parent() {
        fs::create_dir_all(parent)?;
    }
    copy_tree(&snapshot, &live, agent.slug(), CopyPolicy::Snapshot(&[]))?;
    Ok(())
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

    let started = std::time::Instant::now();
    let result = match mode {
        SnapshotMode::Copy => copy_tree(
            &src,
            &dest,
            agent.slug(),
            CopyPolicy::Snapshot(super::exclude::excluded_paths(agent)),
        ),
        SnapshotMode::Move => {
            if fs::rename(&src, &dest).is_ok() {
                // A rename moves the whole tree by definition — there is
                // nothing to count and nothing was filtered.
                Ok(SnapshotStats::default())
            } else {
                // Cross-device rename (EXDEV) — delegate to the extracted
                // helper that logs and surfaces cleanup failures clearly.
                move_dir_cross_device(&src, &dest, agent)
            }
        }
    };
    // Which agent aborted the run only exists here: the CLI's
    // `op_failed` knows the subcommand, not the agent it was on, and a
    // `Move` that failed mid-copy has already deleted part of the source.
    let stats = match result {
        Ok(s) => s,
        Err(e) => {
            if crate::observability::telemetry_gate::is_enabled() {
                tracing::warn!(
                    event = "agent_profile.snapshot_failed",
                    agent = agent.slug(),
                    mode = mode_label(mode),
                    error_kind = io_error_kind(&e),
                    duration_ms = started.elapsed().as_millis() as u64,
                );
            }
            return Err(e.into());
        }
    };
    emit_snapshot_completed(agent, mode, &stats, started.elapsed().as_millis() as u64);
    Ok(())
}

fn mode_label(mode: SnapshotMode) -> &'static str {
    match mode {
        SnapshotMode::Copy => "copy",
        SnapshotMode::Move => "move",
    }
}

/// Bounded telemetry label for an IO failure. `ErrorKind`'s own Debug is
/// bounded too, but the rest of the taxonomy is lowercase-snake.
fn io_error_kind(e: &std::io::Error) -> &'static str {
    use std::io::ErrorKind as K;
    match e.kind() {
        K::NotFound => "not_found",
        K::PermissionDenied => "permission_denied",
        K::AlreadyExists => "already_exists",
        K::InvalidInput | K::InvalidData => "invalid",
        K::Unsupported => "unsupported",
        K::OutOfMemory => "out_of_memory",
        _ => "io",
    }
}

/// One info-level event per agent snapshot. Deliberately info, not debug:
/// the per-path skip events are debug and the file/OTLP sinks floor at
/// info, so without this the whole snapshot is invisible in production.
fn emit_snapshot_completed(
    agent: Agent,
    mode: SnapshotMode,
    stats: &SnapshotStats,
    duration_ms: u64,
) {
    if !crate::observability::telemetry_gate::is_enabled() {
        return;
    }
    tracing::info!(
        event = "agent_profile.snapshot_completed",
        agent = agent.slug(),
        mode = mode_label(mode),
        files_copied = stats.files_copied,
        bytes = stats.bytes,
        subtrees_excluded = stats.subtrees_excluded,
        symlinks_skipped = stats.symlinks_skipped,
        special_skipped = stats.special_skipped,
        duration_ms,
    );
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
            // A half-copied profile is worse than none: `use` would
            // switch an agent to a snapshot missing files it needs, and
            // `create` would then refuse to retry because the dir exists.
            if let Err(e) = clone_profile_tree(&src, &pdir) {
                let _ = fs::remove_dir_all(&pdir);
                return Err(e.into());
            }
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
    profile_name_under_store(&store_root, &resolved)
}

/// Resolve the currently-active profile name for `agent`, honoring the
/// same tier priority everywhere:
/// 1. Env-tier: the config-dir env var if it points into `store_root`.
/// 2. Registry: `profiles.json` active[slug] as fallback.
/// 3. Symlink-tier: the live symlink target's profile name.
///
/// Walks every mechanism the agent exposes so an agent with both Env AND
/// Symlink is honored consistently. Callers must pre-load `registry`
/// and `store_root` (both cheap-to-cache; passing them in avoids
/// per-agent I/O in loops).
///
/// This is the single authoritative implementation — `status::check_preference`
/// and `commands::agents_profile_cmd::save_action` both call this, so the
/// "env → registry → link" ordering can never drift between them.
pub fn resolve_active_profile(
    agent: crate::agents::Agent,
    registry: &super::ProfileRegistry,
    store_root: &Path,
) -> Option<String> {
    for mechanism in agent.profile_mechanisms() {
        match mechanism {
            crate::agents::ProfileMechanism::Env { var } => {
                if let Some(val) = std::env::var_os(var) {
                    let p = std::path::PathBuf::from(val);
                    if let Some(name) = profile_name_under_store(store_root, &p) {
                        return Some(name);
                    }
                }
                if let Some(name) = registry.active.get(agent.slug()) {
                    return Some(name.clone());
                }
            }
            crate::agents::ProfileMechanism::Symlink => {
                if let Some(name) = active_profile_from_link(agent) {
                    return Some(name);
                }
            }
        }
    }
    None
}

/// The profile name in `<store_root>/<profile>/<agent-slug>`, or `None`
/// when `path` doesn't land under the store.
///
/// The canonicalize fallback is load-bearing on macOS, where the store
/// resolves through `/var` → `/private/var` and a plain `strip_prefix`
/// misses. Shared by the symlink-tier reader above and the env-tier one
/// in `status.rs`, which reads the same shape out of `CLAUDE_CONFIG_DIR`.
pub(crate) fn profile_name_under_store(store_root: &Path, path: &Path) -> Option<String> {
    let rel = match path.strip_prefix(store_root) {
        Ok(r) => r.to_path_buf(),
        Err(_) => {
            let root_canon = store_root.canonicalize().ok()?;
            let path_canon = path.canonicalize().ok()?;
            path_canon.strip_prefix(&root_canon).ok()?.to_path_buf()
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

/// Copy one profile dir onto another for `create --from`.
///
/// Per-agent rather than one flat `copy_tree`: the source is a profile,
/// so its top-level entries are agent slugs, and each subtree has to be
/// filtered with *that agent's* denylist. The previous flat copy passed
/// an empty exclusion list on the theory that the source was "already
/// filtered" — untrue for symlink-tier agents, whose snapshot arrives by
/// `fs::rename` and is therefore never filtered at all. Cloning a cursor
/// profile duplicated the entire moved config dir.
fn clone_profile_tree(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    if let Ok(meta) = fs::metadata(src) {
        copy_perms_from_meta(&meta, dst);
    }
    let started = std::time::Instant::now();
    let mut stats = SnapshotStats::default();
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name();
        let from = entry.path();
        let to = dst.join(&name);
        let is_dir = entry.file_type()?.is_dir();
        let agent = name.to_str().and_then(Agent::from_slug);
        match agent {
            Some(agent) if is_dir => {
                let sub = copy_tree(
                    &from,
                    &to,
                    agent.slug(),
                    CopyPolicy::Snapshot(super::exclude::excluded_paths(agent)),
                )?;
                stats.merge(sub);
            }
            // Anything that isn't a recognized agent subdir (registry
            // sidecars, user notes) is profile-level content with no
            // denylist of its own — copy it verbatim.
            _ if is_dir => {
                stats.merge(copy_tree(
                    &from,
                    &to,
                    "<profile>",
                    CopyPolicy::Snapshot(&[]),
                )?);
            }
            // Not a directory: a top-level symlink or file sitting beside
            // the agent subdirs. It goes through the same vetting the
            // recursive walk applies — a bare `fs::copy` here would follow
            // a link out of the profile and choke on a socket.
            _ => copy_entry(
                &entry,
                entry.file_type()?,
                &to,
                src,
                "<profile>",
                CopyPolicy::Snapshot(&[]),
                &mut stats,
            )?,
        }
    }
    // Its own event rather than a `snapshot_completed` row with a
    // sentinel agent: this rolls up every agent in the profile at once,
    // so filing it under `agent = "<profile>"` would put a value in the
    // `agent` dimension that no per-agent query can ever mean.
    if crate::observability::telemetry_gate::is_enabled() {
        tracing::info!(
            event = "agent_profile.clone_completed",
            files_copied = stats.files_copied,
            bytes = stats.bytes,
            subtrees_excluded = stats.subtrees_excluded,
            symlinks_skipped = stats.symlinks_skipped,
            special_skipped = stats.special_skipped,
            duration_ms = started.elapsed().as_millis() as u64,
        );
    }
    Ok(())
}

/// Fallback for a cross-device rename (EXDEV): copy then remove.
/// `agent` is only used for telemetry — the operation is generic.
fn move_dir_cross_device(src: &Path, dst: &Path, agent: Agent) -> std::io::Result<SnapshotStats> {
    if crate::observability::telemetry_gate::is_enabled() {
        tracing::debug!(
            event = "agent_profile.snapshot_cross_device",
            agent = agent.slug(),
        );
    }
    // `Relocate`: this stands in for a rename and the source is deleted
    // below, so nothing may be dropped — not excluded paths, and not
    // symlinks the snapshot path would have refused.
    let stats = copy_tree(src, dst, agent.slug(), CopyPolicy::Relocate)?;
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
    Ok(stats)
}

/// Recursive copy preserving permission bits.
///
/// Under `CopyPolicy::Snapshot`, symlinks are recreated as links (never
/// followed) but only when the target is relative AND stays within
/// `snapshot_root`; absolute or escaping links are refused and emitted as
/// `agent_profile.symlink_skipped`. Under `CopyPolicy::Relocate` every
/// link is recreated verbatim — the source is about to be deleted, so
/// refusing one would destroy it rather than leave it behind.
///
/// `agent_slug` is passed in for telemetry only.
fn copy_tree(
    src: &Path,
    dst: &Path,
    agent_slug: &str,
    policy: CopyPolicy,
) -> std::io::Result<SnapshotStats> {
    let mut stats = SnapshotStats::default();
    copy_tree_inner(src, dst, src, agent_slug, policy, &mut stats)?;
    Ok(stats)
}

fn copy_tree_inner(
    src: &Path,
    dst: &Path,
    snapshot_root: &Path,
    agent_slug: &str,
    policy: CopyPolicy,
    stats: &mut SnapshotStats,
) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    // Use the metadata already available for `src` to set dst perms.
    if let Ok(meta) = fs::metadata(src) {
        copy_perms_from_meta(&meta, dst);
    }
    let excludes: &[&'static str] = match policy {
        CopyPolicy::Snapshot(e) => e,
        CopyPolicy::Relocate => &[],
    };
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        // `file_type()` uses symlink_metadata semantics on all platforms
        // (never follows); `metadata()` is fetched separately for perms.
        let ft = entry.file_type()?; // never follows symlinks
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if !excludes.is_empty()
            && let Ok(rel) = from.strip_prefix(snapshot_root)
            && let Some(rel) = rel.to_str()
            && let Some(pattern) =
                super::exclude::matched_pattern(&rel.replace('\\', "/"), excludes)
        {
            stats.subtrees_excluded += 1;
            if crate::observability::telemetry_gate::is_enabled() {
                tracing::debug!(
                    event = "agent_profile.path_excluded",
                    agent = agent_slug,
                    pattern,
                    "skipped non-identity path during snapshot copy",
                );
            }
            continue;
        }
        // `ft` never follows, so a symlink-to-dir is not `is_dir` here and
        // falls through to `copy_entry`'s link handling.
        if ft.is_dir() {
            copy_tree_inner(&from, &to, snapshot_root, agent_slug, policy, stats)?;
        } else {
            copy_entry(&entry, ft, &to, snapshot_root, agent_slug, policy, stats)?;
        }
    }
    Ok(())
}

/// Copy one non-directory entry under `policy`.
///
/// Split out of the recursive walk because `create --from` needs the exact
/// same vetting for the entries sitting beside a profile's agent subdirs:
/// `fs::copy` follows symlinks, so the flat version of this leaked file
/// contents from outside the profile and aborted on a socket.
fn copy_entry(
    entry: &fs::DirEntry,
    ft: std::fs::FileType,
    to: &Path,
    snapshot_root: &Path,
    agent_slug: &str,
    policy: CopyPolicy,
    stats: &mut SnapshotStats,
) -> std::io::Result<()> {
    let from = entry.path();
    if ft.is_symlink() {
        // Absolute or escaping links are hostile *when importing* into
        // the store. When relocating they are the user's own data and
        // must survive the impending delete of the source.
        let refusal = match policy {
            CopyPolicy::Relocate => None,
            CopyPolicy::Snapshot(_) => symlink_refusal_reason(&from, snapshot_root),
        };
        match refusal {
            None => recreate_symlink(&from, to)?,
            Some(reason) => {
                stats.symlinks_skipped += 1;
                if crate::observability::telemetry_gate::is_enabled() {
                    tracing::warn!(
                        event = "agent_profile.symlink_skipped",
                        agent = agent_slug,
                        reason,
                        "skipped unsafe symlink during snapshot copy",
                    );
                }
            }
        }
    } else if !ft.is_file() {
        // Sockets / FIFOs / device nodes are live IPC endpoints, not
        // config (e.g. `~/.codex/ipc/ipc.sock`). `fs::copy` fails on
        // them with ENOTSUP, which would abort the whole snapshot.
        // There is no way to copy one, so a relocate loses it — say so
        // at warn level there, since the source is about to be removed.
        stats.special_skipped += 1;
        if crate::observability::telemetry_gate::is_enabled() {
            let kind = special_file_kind(&ft);
            match policy {
                CopyPolicy::Relocate => tracing::warn!(
                    event = "agent_profile.special_file_skipped",
                    agent = agent_slug,
                    kind,
                    relocating = true,
                    "non-regular file cannot be relocated and will not survive the move",
                ),
                CopyPolicy::Snapshot(_) => tracing::debug!(
                    event = "agent_profile.special_file_skipped",
                    agent = agent_slug,
                    kind,
                    relocating = false,
                    "skipped non-regular file during snapshot copy",
                ),
            }
        }
    } else {
        fs::copy(&from, to)?;
        stats.files_copied += 1;
        // Use symlink_metadata (via entry.metadata()) for the already-
        // fetched DirEntry stats — avoids a second stat call (D-2).
        if let Ok(meta) = entry.metadata() {
            copy_perms_from_meta(&meta, to);
            stats.bytes += meta.len();
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

/// `None` when the symlink at `link` (inside `source_root`) has a relative
/// target that stays within `source_root`; otherwise a bounded reason for
/// the refusal, which `agent_profile.symlink_skipped` carries instead of a
/// path. Purely lexical; never follows.
fn symlink_refusal_reason(link: &Path, source_root: &Path) -> Option<&'static str> {
    let Ok(target) = fs::read_link(link) else {
        return Some("unreadable");
    };
    // Absolute targets always point outside the snapshot source.
    if target.is_absolute() {
        return Some("absolute_target");
    }
    // Resolve the relative target against the link's parent dir.
    let Some(parent) = link.parent() else {
        return Some("escapes_root");
    };
    let resolved = parent.join(&target);
    // Walk the components and track depth; any `..` that would go above
    // `source_root` is a traversal escape.
    let mut depth: isize = 0;
    // Compute the depth of the link parent relative to source_root to
    // know how many `..` steps are allowed.
    let Ok(base_relative) = parent.strip_prefix(source_root) else {
        return Some("escapes_root"); // link is not under source_root at all
    };
    let base_depth = base_relative.components().count() as isize;
    for comp in target.components() {
        match comp {
            std::path::Component::ParentDir => {
                depth -= 1;
                if depth < -base_depth {
                    return Some("escapes_root");
                }
            }
            std::path::Component::Normal(_) => depth += 1,
            _ => {}
        }
    }
    // Final sanity: the resolved path must still start with source_root
    // (handles edge cases in the depth arithmetic).
    if resolved.starts_with(source_root) {
        None
    } else {
        Some("escapes_root")
    }
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

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn snapshot_drops_transcripts_but_keeps_config() {
        let _home = JarvyHomeGuard::new();
        let src = Agent::ClaudeCode.config_dir().unwrap();
        fs::create_dir_all(src.join("projects/repo")).unwrap();
        fs::write(src.join("projects/repo/session.jsonl"), "transcript").unwrap();
        fs::create_dir_all(src.join("plugins/cache/x")).unwrap();
        fs::write(src.join("plugins/cache/x/blob"), "clone").unwrap();
        fs::create_dir_all(src.join("plugins")).unwrap();
        fs::write(src.join("plugins/installed_plugins.json"), "{}").unwrap();
        fs::write(src.join("settings.json"), "{}").unwrap();

        snapshot_agent(Agent::ClaudeCode, "lean", SnapshotMode::Copy).unwrap();

        let snap = profile_dir("lean").unwrap().join("claude-code");
        assert!(snap.join("settings.json").is_file());
        assert!(
            snap.join("plugins/installed_plugins.json").is_file(),
            "the identity half of plugins/ must survive"
        );
        assert!(
            !snap.join("projects").exists(),
            "transcripts must be skipped"
        );
        assert!(!snap.join("plugins/cache").exists());
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

    /// `CopyPolicy::Relocate` removes its source once the copy lands, so
    /// anything it skipped would be destroyed rather than left behind.
    /// The EXDEV fallback therefore carries denylisted paths and the
    /// absolute symlinks a snapshot would refuse.
    #[cfg(unix)]
    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn move_dir_cross_device_relocates_everything_unfiltered() {
        let _home = JarvyHomeGuard::new();
        let src = Agent::Cursor.config_dir().unwrap();
        // `projects` is on cursor's denylist; a snapshot would drop it.
        fs::create_dir_all(src.join("projects/repo")).unwrap();
        fs::write(src.join("projects/repo/state.json"), "scratch").unwrap();
        fs::write(src.join("mcp.json"), "{}").unwrap();
        // Absolute target: refused under Snapshot, mandatory under Relocate.
        std::os::unix::fs::symlink(src.join("mcp.json"), src.join("abs-link")).unwrap();
        let dst = src.parent().unwrap().join("relocated");

        let stats = move_dir_cross_device(&src, &dst, Agent::Cursor).unwrap();

        assert_eq!(
            fs::read_to_string(dst.join("projects/repo/state.json")).unwrap(),
            "scratch",
            "relocate must not apply the denylist"
        );
        assert!(
            fs::symlink_metadata(dst.join("abs-link"))
                .unwrap()
                .file_type()
                .is_symlink(),
            "absolute link must be recreated, not dropped"
        );
        assert_eq!(stats.subtrees_excluded, 0);
        assert_eq!(stats.symlinks_skipped, 0);
        assert!(!src.exists(), "source is removed after a successful move");
    }

    /// Each top-level dir in a profile is resolved to its agent and
    /// filtered with *that* agent's denylist. Codex keeps `projects`
    /// (only `sessions` is transcript state there), so applying
    /// claude-code's rules across the whole profile would lose data.
    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn create_from_applies_each_agents_own_denylist() {
        let _home = JarvyHomeGuard::new();
        let a = create_profile("a", None).unwrap();
        let cc = a.join("claude-code");
        fs::create_dir_all(cc.join("projects/repo")).unwrap();
        fs::write(cc.join("projects/repo/session.jsonl"), "transcript").unwrap();
        fs::write(cc.join("settings.json"), "{}").unwrap();
        let cx = a.join("codex");
        fs::create_dir_all(cx.join("sessions/s")).unwrap();
        fs::write(cx.join("sessions/s/log.jsonl"), "transcript").unwrap();
        fs::create_dir_all(cx.join("projects")).unwrap();
        fs::write(cx.join("projects/notes.md"), "keep").unwrap();
        fs::write(cx.join("config.toml"), "").unwrap();

        let b = create_profile("b", Some("a")).unwrap();

        assert!(b.join("claude-code/settings.json").is_file());
        assert!(
            !b.join("claude-code/projects").exists(),
            "claude-code transcripts must not seed the new profile"
        );
        assert!(!b.join("codex/sessions").exists());
        assert!(
            b.join("codex/projects/notes.md").is_file(),
            "claude-code's denylist must not reach codex"
        );
        assert!(b.join("codex/config.toml").is_file());
    }

    /// The clone loop used to `fs::copy` top-level non-directory entries,
    /// which follows links — a stray symlink beside the agent dirs pulled
    /// content from outside the profile into the new one.
    #[cfg(unix)]
    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn create_from_does_not_follow_top_level_symlink() {
        let _home = JarvyHomeGuard::new();
        let a = create_profile("a", None).unwrap();
        let outside = a.parent().unwrap().join("outside.txt");
        fs::write(&outside, "sensitive").unwrap();
        std::os::unix::fs::symlink(&outside, a.join("stray")).unwrap();
        fs::write(a.join("notes.md"), "mine").unwrap();

        let b = create_profile("b", Some("a")).unwrap();

        assert!(
            fs::symlink_metadata(b.join("stray")).is_err(),
            "escaping link must be refused, not followed"
        );
        assert_eq!(fs::read_to_string(b.join("notes.md")).unwrap(), "mine");
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

    #[cfg(unix)]
    #[test]
    fn symlink_refusal_absolute_link_refused() {
        // Unix-only: Windows symlink creation requires either
        // Developer Mode or elevated privileges, and `symlink_refusal_reason`
        // reads the link target from disk (returns "unreadable" for a
        // non-symlink path). Coverage of the absolute-target refusal on
        // Windows lives in the integration path via the real snapshot code.
        let tmp = tempfile::tempdir().unwrap();
        let source_root = tmp.path().join("src");
        fs::create_dir_all(&source_root).unwrap();
        let link = source_root.join("abs-link");
        std::os::unix::fs::symlink("/etc/passwd", &link).unwrap();
        assert_eq!(
            symlink_refusal_reason(&link, &source_root),
            Some("absolute_target"),
            "absolute symlink must be refused"
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlink_refusal_escaping_relative_link_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let source_root = tmp.path().join("src");
        fs::create_dir_all(source_root.join("sub")).unwrap();
        // A relative link that escapes above source_root.
        let link = source_root.join("escape-link");
        std::os::unix::fs::symlink("../../outside", &link).unwrap();
        assert_eq!(
            symlink_refusal_reason(&link, &source_root),
            Some("escapes_root"),
            "escaping symlink must be refused"
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlink_refusal_in_tree_relative_link_preserved() {
        let tmp = tempfile::tempdir().unwrap();
        let source_root = tmp.path().join("src");
        fs::create_dir_all(&source_root).unwrap();
        fs::write(source_root.join("real.txt"), "x").unwrap();
        // A relative link pointing at a sibling file — in-tree.
        let link = source_root.join("alias.txt");
        std::os::unix::fs::symlink("real.txt", &link).unwrap();
        assert_eq!(
            symlink_refusal_reason(&link, &source_root),
            None,
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

    // ── save / restore tests ───────────────────────────────────────────

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn save_env_tier_refreshes_snapshot_from_live() {
        let _home = JarvyHomeGuard::new();
        create_profile("work", None).unwrap();
        let live = Agent::ClaudeCode.config_dir().unwrap();
        fs::create_dir_all(&live).unwrap();
        fs::write(live.join("settings.json"), "v1").unwrap();
        // First snapshot lands via save.
        assert_eq!(
            save_agent(Agent::ClaudeCode, "work").unwrap(),
            SaveOutcome::EnvCopied
        );
        let snap = profile_dir("work").unwrap().join("claude-code");
        assert_eq!(
            fs::read_to_string(snap.join("settings.json")).unwrap(),
            "v1"
        );

        // Mutate live, save again: snapshot must mirror the new state.
        fs::write(live.join("settings.json"), "v2").unwrap();
        fs::write(live.join("new.txt"), "added").unwrap();
        assert_eq!(
            save_agent(Agent::ClaudeCode, "work").unwrap(),
            SaveOutcome::EnvCopied
        );
        assert_eq!(
            fs::read_to_string(snap.join("settings.json")).unwrap(),
            "v2"
        );
        assert_eq!(fs::read_to_string(snap.join("new.txt")).unwrap(), "added");
    }

    #[cfg(unix)]
    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn save_symlink_tier_is_noop_when_managed() {
        let _home = JarvyHomeGuard::new();
        // init snapshots cursor and links `~/.cursor` back into the store.
        fs::create_dir_all(Agent::Cursor.config_dir().unwrap()).unwrap();
        init_snapshot("default", None).unwrap();

        let outcome = save_agent(Agent::Cursor, "default").unwrap();
        assert_eq!(outcome, SaveOutcome::SymlinkNoOp);
        // Live link still points into the store, untouched.
        let live = Agent::Cursor.config_dir().unwrap();
        let store_root = crate::paths::agent_profiles_dir().unwrap();
        assert!(link_points_into(&live, &store_root));
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn save_symlink_tier_reports_unmanaged() {
        let _home = JarvyHomeGuard::new();
        create_profile("work", None).unwrap();
        // Real dir at ~/.cursor — not jarvy's to snapshot.
        fs::create_dir_all(Agent::Cursor.config_dir().unwrap()).unwrap();
        assert!(matches!(
            save_agent(Agent::Cursor, "work"),
            Err(ProfileError::UnmanagedDir(_))
        ));
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn save_missing_profile_errors() {
        let _home = JarvyHomeGuard::new();
        let live = Agent::ClaudeCode.config_dir().unwrap();
        fs::create_dir_all(&live).unwrap();
        assert!(matches!(
            save_agent(Agent::ClaudeCode, "ghost"),
            Err(ProfileError::ProfileNotFound(_))
        ));
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn restore_env_tier_copies_snapshot_over_live() {
        let _home = JarvyHomeGuard::new();
        create_profile("work", None).unwrap();
        // Snapshot has X ...
        let snap = profile_dir("work").unwrap().join("claude-code");
        fs::create_dir_all(&snap).unwrap();
        fs::write(snap.join("settings.json"), "snapshot-v").unwrap();
        // ... live has Y (a completely different file set).
        let live = Agent::ClaudeCode.config_dir().unwrap();
        fs::create_dir_all(&live).unwrap();
        fs::write(live.join("stale.txt"), "gone").unwrap();

        restore_agent(Agent::ClaudeCode, "work").unwrap();

        // Snapshot's content is now live; stale files are gone.
        assert_eq!(
            fs::read_to_string(live.join("settings.json")).unwrap(),
            "snapshot-v"
        );
        assert!(!live.join("stale.txt").exists());
    }

    #[cfg(unix)]
    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn restore_symlink_tier_repoints_link() {
        let _home = JarvyHomeGuard::new();
        // Two profiles both carrying a cursor snapshot.
        create_profile("a", None).unwrap();
        create_profile("b", None).unwrap();
        let a = profile_dir("a").unwrap().join("cursor");
        fs::create_dir_all(&a).unwrap();
        fs::write(a.join("marker.txt"), "a-side").unwrap();
        let b = profile_dir("b").unwrap().join("cursor");
        fs::create_dir_all(&b).unwrap();
        fs::write(b.join("marker.txt"), "b-side").unwrap();

        // Start pointed at a.
        let live = Agent::Cursor.config_dir().unwrap();
        std::os::unix::fs::symlink(&a, &live).unwrap();
        // Restore b: link re-points, marker reads through the link changes.
        restore_agent(Agent::Cursor, "b").unwrap();
        assert_eq!(fs::read_link(&live).unwrap(), b);
        assert_eq!(
            fs::read_to_string(live.join("marker.txt")).unwrap(),
            "b-side"
        );
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn restore_missing_snapshot_errors() {
        let _home = JarvyHomeGuard::new();
        create_profile("work", None).unwrap();
        assert!(matches!(
            restore_agent(Agent::ClaudeCode, "work"),
            Err(ProfileError::AgentNotSnapshotted { .. })
        ));
    }

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

    /// Env-tier resolution must prefer a live env var over a stale
    /// registry entry: two shells can differ, and the whole point of
    /// the env tier is that THIS terminal's env var is authoritative.
    /// The registry only kicks in when the var is absent.
    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn resolve_active_profile_env_var_wins_over_registry() {
        let _home = JarvyHomeGuard::new();
        // Seed the store: two profiles, snapshots for claude-code in both.
        create_profile("work", None).unwrap();
        create_profile("play", None).unwrap();
        let play_snap = crate::paths::agent_profiles_dir()
            .unwrap()
            .join("play")
            .join("claude-code");
        fs::create_dir_all(&play_snap).unwrap();

        // Registry says the agent is on "work".
        let mut active = std::collections::BTreeMap::new();
        active.insert("claude-code".to_string(), "work".to_string());
        let reg = ProfileRegistry {
            active,
            ..Default::default()
        };
        let store_root = crate::paths::agent_profiles_dir().unwrap();

        // Point CLAUDE_CONFIG_DIR at the "play" snapshot — env must win.
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var("CLAUDE_CONFIG_DIR", &play_snap);
        }
        let resolved = resolve_active_profile(Agent::ClaudeCode, &reg, &store_root);
        // Clean up before asserting so a panic doesn't leak env state.
        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("CLAUDE_CONFIG_DIR");
        }
        assert_eq!(
            resolved.as_deref(),
            Some("play"),
            "env var must win over registry (env={:?}, registry=work)",
            play_snap
        );

        // With the env var gone, the registry entry becomes the fallback.
        let resolved = resolve_active_profile(Agent::ClaudeCode, &reg, &store_root);
        assert_eq!(
            resolved.as_deref(),
            Some("work"),
            "registry must be the fallback when the env var is unset"
        );
    }
}
