//! `jarvy agents profile <action>` command handler (PRD-058 v1).
//!
//! CLI surface over the `agent_profiles` core: init / create / use /
//! list / status / delete. Env-tier agents (claude-code, codex) switch
//! per-terminal via `export` lines; the symlink tier (cursor in v1)
//! switches globally and only under `--global`. Profile NAMES never
//! reach telemetry — events carry counts and bounded slugs only.

use std::fmt::Write as _;
use std::path::PathBuf;
use std::time::Instant;

use serde::Serialize;

use crate::agent_profiles::{
    self, ProfileError, ProfileRegistry, ProfileStatusReport, SaveOutcome, SwitchPlan,
};
use crate::agents::Agent;
use crate::cli::{AgentsAction, ProfileAction};
use crate::error_codes::CONFIG_ERROR;
use crate::output::{Outputable, print_and_exit};

pub fn run_agents_profile(action: &AgentsAction) -> i32 {
    match action {
        AgentsAction::Profile { action } => match action {
            ProfileAction::Init { agents } => init_action(agents.as_deref()),
            ProfileAction::Create { name, from, agents } => {
                create_action(name, from.as_deref(), agents.as_deref())
            }
            ProfileAction::Use {
                name,
                agents,
                global,
                print_env,
                force,
            } => use_action(name, agents.as_deref(), *global, *print_env, *force),
            ProfileAction::List { output_format } => list_action(output_format),
            ProfileAction::Status { output_format } => status_action(output_format),
            ProfileAction::Delete { name } => delete_action(name),
            ProfileAction::Save { name, agents } => save_action(name.as_deref(), agents.as_deref()),
            ProfileAction::Restore {
                name,
                agents,
                force,
            } => restore_action(name, agents.as_deref(), *force),
            ProfileAction::CheckCwd { session_id, quiet } => {
                agent_profiles::cwd_hint::run(session_id.as_deref(), *quiet)
            }
        },
    }
}

// ---------------------------------------------------------------------------
// init

fn init_action(agents_raw: Option<&str>) -> i32 {
    let started = Instant::now();
    let explicit = agents_raw.is_some();
    let only = match parse_optional_agents(agents_raw) {
        Ok(v) => v,
        Err(code) => return code,
    };
    // Ensure the 'default' profile dir exists even when no agents are
    // installed — an empty store must still be listable. Idempotent.
    if let Err(e) = agent_profiles::create_profile("default", None)
        && !matches!(e, ProfileError::ProfileExists(_))
    {
        return fail("init", "create_profile", &e);
    }
    let snapshotted = match agent_profiles::init_snapshot("default", only.as_deref()) {
        Ok(agents) => agents,
        Err(e) => return fail("init", "snapshot", &e),
    };

    let mut registry = match ProfileRegistry::load() {
        Ok(r) => r,
        Err(e) => return fail("init", "registry_load", &e),
    };
    // Mirrors `use`: a narrowed run only speaks for the agents it named,
    // so it must not repoint every new shell at 'default'. Claiming the
    // default on an unnarrowed run is still correct — that IS the full set.
    let claim_default = !explicit || registry.default_profile.is_none();
    if claim_default {
        registry.default_profile = Some("default".to_string());
    }
    for agent in &snapshotted {
        registry
            .active
            .insert(agent.slug().to_string(), "default".to_string());
    }
    // Reconcile symlink-tier registry entries with the actual on-disk
    // link targets — fixes stale entries from a prior `use --global`.
    for &agent in Agent::ALL {
        if !agent.is_symlink_tier() {
            continue;
        }
        match agent_profiles::store::active_profile_from_link(agent) {
            Some(p) => {
                registry.active.insert(agent.slug().to_string(), p);
            }
            None => {
                registry.active.remove(agent.slug());
            }
        }
    }
    if let Err(e) = registry.save() {
        return fail("init", "registry_save", &e);
    }

    if snapshotted.is_empty() {
        println!("No installed agents found — created empty profile 'default'.");
    } else {
        println!(
            "Snapshotted {} agent(s) into profile 'default':",
            snapshotted.len()
        );
        for agent in &snapshotted {
            println!("  {} ({})", agent.slug(), tier_label(*agent));
        }
    }
    if claim_default {
        println!("Default profile set to 'default'.");
    } else {
        eprintln!(
            "note: --agents was specified; default profile unchanged (snapshotted listed agents only)"
        );
    }
    emit_created(
        "init",
        false,
        explicit,
        started.elapsed().as_millis() as u64,
        snapshotted.len(),
    );
    0
}

// ---------------------------------------------------------------------------
// create

fn create_action(name: &str, from: Option<&str>, agents_raw: Option<&str>) -> i32 {
    let started = Instant::now();
    let explicit = agents_raw.is_some();
    let only = match parse_optional_agents(agents_raw) {
        Ok(v) => v,
        Err(code) => return code,
    };

    // Use the returned PathBuf from create_profile directly (finding 11).
    let pdir = match agent_profiles::create_profile(name, from) {
        Ok(p) => p,
        Err(e) => return fail("create", "create_profile", &e),
    };

    // `--agents` narrows a seeded copy: drop snapshots outside the list.
    let mut agent_count = 0usize;
    for &agent in Agent::ALL {
        let sub = pdir.join(agent.slug());
        if !sub.exists() {
            continue;
        }
        if only.as_ref().is_some_and(|list| !list.contains(&agent)) {
            if let Err(e) = std::fs::remove_dir_all(&sub) {
                eprintln!("Error: pruning {} from '{name}': {e}", agent.slug());
                return fail("create", "prune", &ProfileError::Io(e));
            }
        } else {
            agent_count += 1;
        }
    }

    match from {
        Some(src) => {
            println!("Created profile '{name}' from '{src}' ({agent_count} agent snapshot(s)).")
        }
        None => {
            println!("Created empty profile '{name}'.");
            // Without `save` (v1.1) an empty profile has no route to
            // becoming usable, so `use` would fail later with no hint
            // that `--from` was the missing step.
            eprintln!(
                "Note: '{name}' holds no agent snapshots yet, so \
                 `jarvy agents profile use {name}` will fail. Seed it with \
                 `--from <existing>`."
            );
        }
    }
    emit_created(
        "create",
        from.is_some(),
        explicit,
        started.elapsed().as_millis() as u64,
        agent_count,
    );
    0
}

// ---------------------------------------------------------------------------
// use

fn use_action(
    name: &str,
    agents_raw: Option<&str>,
    global: bool,
    print_env: bool,
    force: bool,
) -> i32 {
    let started = Instant::now();
    let explicit = agents_raw.is_some();
    let requested: Vec<Agent> = match parse_optional_agents(agents_raw) {
        Ok(Some(list)) => list,
        Ok(None) => {
            // Unfiltered use: skip agents that lack a snapshot in this
            // profile. All six agents are switchable, but requiring
            // every agent to have a snapshot would break "use work"
            // for users who only ever snapshotted claude-code + codex.
            // Explicit --agents still errors on a missing snapshot —
            // the user asked for it, so silence would hide the mistake.
            match agent_profiles::store::profile_dir(name) {
                Ok(pdir) => Agent::ALL
                    .iter()
                    .copied()
                    .filter(|a| pdir.join(a.slug()).is_dir())
                    .collect(),
                Err(_) => Agent::ALL.to_vec(), // let plan_use report the profile-not-found
            }
        }
        Err(code) => return code,
    };

    // With --print-env, stdout carries ONLY export lines (shell-eval'd);
    // every human message goes to stderr.
    let say = |msg: &str| {
        if print_env {
            eprintln!("{msg}");
        } else {
            println!("{msg}");
        }
    };

    // Every agent is switchable now, so the tier check is gone. Env-
    // tier switches don't need the live config dir to pre-exist (the
    // agent creates it on first read); symlink-tier `apply_symlink_repoint`
    // handles the NotFound case by creating the link fresh. So the old
    // storage-only skip has no functional replacement — take every
    // requested agent through the planner. `agent_profile.use_unsupported`
    // was retired with this change: no code path emits it anymore
    // (documented in the taxonomy as dead).
    let targets: Vec<Agent> = requested;
    if targets.is_empty() {
        // Only reachable when parse_optional_agents returned Ok(None) →
        // Agent::ALL, which is never empty — or when a future
        // filtering step drains it. Left in as a defensive floor.
        eprintln!(
            "Error: no switchable agents selected — install at least one agent \
             (claude-code, codex, cursor, windsurf, cline, continue)"
        );
        return CONFIG_ERROR;
    }

    let symlink_requested: Vec<Agent> = targets
        .iter()
        .copied()
        .filter(|a| a.is_symlink_tier())
        .collect();

    let plans = match agent_profiles::plan_use(name, &targets, global) {
        Ok(p) => p,
        Err(e) => return fail("use", "plan", &e),
    };

    // Read before anything switches: `first_switch` means "this agent had
    // no prior registry entry". Telemetry-only, so a load failure degrades
    // to an empty map rather than aborting a switch that can still succeed.
    let pre_active: std::collections::BTreeMap<String, String> = ProfileRegistry::load()
        .map(|r| r.active)
        .unwrap_or_default();

    let mut exports: Vec<(&'static str, PathBuf)> = Vec::new();
    let mut repointed: Vec<Agent> = Vec::new();
    // Track agents refused by the running-IDE guard so the "nothing
    // switched" error at the end is accurate and the registry stays
    // consistent with what actually landed on disk.
    let mut ide_blocked: Vec<Agent> = Vec::new();
    for plan in plans {
        match plan {
            SwitchPlan::EnvExports(vars) => exports.extend(vars),
            SwitchPlan::SymlinkRepoint { agent, target } => {
                if global {
                    // Fail-safe: probe failures return false, so the
                    // guard only fires on a positive detection. `--force`
                    // is documented as the escape hatch — the events
                    // capture WHICH path we took either way.
                    if agent_profiles::probe::is_ide_running(agent) {
                        if !force {
                            emit_ide_running(agent, false);
                            eprintln!(
                                "note: {} is running — refusing to re-point its config dir. \
                                 Quit the editor and rerun, or pass --force.",
                                agent.slug()
                            );
                            ide_blocked.push(agent);
                            continue;
                        } else {
                            emit_ide_running(agent, true);
                            eprintln!(
                                "warn: {} is running — swapping under --force. \
                                 Restart the editor to pick up the new profile.",
                                agent.slug()
                            );
                        }
                    }
                    if let Err(e) = agent_profiles::apply_symlink_repoint(agent, &target) {
                        return fail("use", "symlink_repoint", &e);
                    }
                    // Emitted here rather than with the env tier at the end:
                    // the live symlink is already re-pointed, so a later
                    // failure (registry write) must not erase the record of
                    // a switch the user's IDE is already seeing.
                    emit_switched_one(
                        agent,
                        "symlink",
                        "global",
                        explicit,
                        started.elapsed().as_millis() as u64,
                        &pre_active,
                    );
                    repointed.push(agent);
                }
                // !global: covered by the stderr notice below.
            }
        }
    }

    let export_block = match format_exports(&exports) {
        Ok(s) => s,
        Err(e) => return fail("use", "format_exports", &e),
    };
    if print_env {
        if !export_block.is_empty() {
            print!("{export_block}");
        }
    } else if exports.is_empty() {
        say("No env-tier exports for this selection.");
    } else {
        say(&format!(
            "Run (or eval) these in your shell to activate '{name}' for this terminal:"
        ));
        print!("{export_block}");
    }

    if !symlink_requested.is_empty() && !global {
        let slugs = slug_list(&symlink_requested);
        eprintln!(
            "note: symlink-tier agents ({slugs}) switch globally — rerun with --global to apply"
        );
    }

    if global {
        for agent in &repointed {
            say(&format!(
                "Switched {} to '{name}' globally (all terminals).",
                agent.slug()
            ));
        }
        let mut registry = match ProfileRegistry::load() {
            Ok(r) => r,
            Err(e) => return fail("use", "registry_load", &e),
        };
        // Skip ide-blocked agents: no repoint happened, so recording
        // them as active would misrepresent the on-disk state and
        // silence the setup-hint drift signal until the next `init`.
        for agent in &targets {
            if ide_blocked.contains(agent) {
                continue;
            }
            registry
                .active
                .insert(agent.slug().to_string(), name.to_string());
        }
        // Only update default_profile when the user targeted the full
        // default set (no explicit --agents narrowing). A partial switch
        // would set default to a profile that doesn't apply to all agents,
        // causing new shells to get mismatched env vs symlink profiles.
        //
        // Also skip the default claim when a running IDE blocked one of
        // the symlink targets — the swap is incomplete, so pointing
        // future shells at a profile that doesn't fully apply would
        // reintroduce the same drift the guard prevented.
        if explicit {
            eprintln!(
                "note: --agents was specified; default profile unchanged (applied to listed agents only)"
            );
        } else if ide_blocked.is_empty() {
            registry.default_profile = Some(name.to_string());
            say(&format!("Default profile set to '{name}'."));
        } else {
            eprintln!(
                "note: default profile unchanged (some agents were skipped by the running-IDE guard)"
            );
        }
        if let Err(e) = registry.save() {
            return fail("use", "registry_save", &e);
        }
    }
    // Env tier only — the symlink rows were emitted at re-point time.
    emit_switched_env(
        &targets,
        global,
        explicit,
        started.elapsed().as_millis() as u64,
        &pre_active,
    );

    // Detect cursor-only symlink-tier targeting without --global: a valid
    // completion that produced neither exports nor re-points — emit a noop note.
    let produced_exports = !exports.is_empty();
    let produced_repoints = !repointed.is_empty();
    // `use_noop` is the "you needed --global" breadcrumb. Suppress it
    // when --global WAS passed and the running-IDE guard is what
    // stopped the swap — the reason is `ide_running`, not
    // `symlink_tier_needs_global`, and the per-agent `ide_running`
    // event already documents that path.
    let ide_only_noop = global && !ide_blocked.is_empty();
    if !produced_exports
        && !produced_repoints
        && !symlink_requested.is_empty()
        && !ide_only_noop
        && crate::observability::telemetry_gate::is_enabled()
    {
        let slugs = slug_list(&symlink_requested);
        tracing::info!(
            event = "agent_profile.use_noop",
            reason = "symlink_tier_needs_global",
            agents = slugs.as_str(),
            "symlink-tier agents require --global to switch"
        );
    }

    // The running-IDE guard fired for every symlink target and no env
    // exports were produced either: nothing switched, so surface this as
    // a config error. Registry state is unchanged for blocked agents.
    if global && !ide_blocked.is_empty() && repointed.is_empty() && exports.is_empty() {
        eprintln!(
            "Error: no agents switched — every requested symlink-tier agent \
             is running. Quit the editor(s) and rerun, or pass --force."
        );
        return CONFIG_ERROR;
    }

    0
}

/// One `agent_profile.ide_running` event per running-IDE detection.
/// `forced` distinguishes the refused path (`forced = false`) from the
/// override path (`forced = true`), keeping the mechanism bounded and
/// the two ratios queryable independently. Refusals emit at `warn`
/// (a swap the user asked for did not happen); `--force` overrides
/// emit at `info` (the user knowingly bypassed a guardrail). Field
/// name is `agent` — uniform with every other `agent_profile.*` event
/// (`switched`, `snapshot_failed`, `symlink_skipped`, `agent_absent`,
/// `path_excluded`, `special_file_skipped`, `unmanaged_dir`).
fn emit_ide_running(agent: Agent, forced: bool) {
    if crate::observability::telemetry_gate::is_enabled() {
        if forced {
            tracing::info!(
                event = "agent_profile.ide_running",
                agent = agent.slug(),
                forced = true,
            );
        } else {
            tracing::warn!(
                event = "agent_profile.ide_running",
                agent = agent.slug(),
                forced = false,
            );
        }
    }
}

/// Bash-syntax export lines for the env tier. Paths are double-quoted
/// with `"` `\` `$` `` ` `` escaped so `eval`-ing the output can't be
/// tricked by a hostile path segment. Returns an error if any path
/// contains control bytes (< 0x20) — embedding a newline inside a
/// double-quoted shell string would split the export line and execute
/// the remainder as a shell command.
fn format_exports(exports: &[(&'static str, PathBuf)]) -> Result<String, ProfileError> {
    let mut out = String::new();
    for (var, path) in exports {
        // Refuse paths with control bytes: POSIX double-quoted strings
        // treat `\n` literally as backslash-n, so escaping-as-literal
        // would corrupt the value. Refusing is the only safe option.
        let display = path.display().to_string();
        if display.chars().any(|c| (c as u32) < 0x20) {
            return Err(ProfileError::InvalidName(format!(
                "path for {var} contains control bytes and cannot be safely exported"
            )));
        }
        let _ = write!(out, "export {var}=\"");
        for c in display.chars() {
            if matches!(c, '"' | '\\' | '$' | '`') {
                out.push('\\');
            }
            out.push(c);
        }
        let _ = writeln!(out, "\"");
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// list / status

#[derive(Serialize)]
struct ProfileListResult {
    profiles: Vec<String>,
    default_profile: Option<String>,
}

impl Outputable for ProfileListResult {
    fn to_human(&self) -> String {
        if self.profiles.is_empty() {
            return "No profiles yet. Run `jarvy agents profile init` to snapshot your \
                    current agent configs as 'default'."
                .to_string();
        }
        let mut out = String::from("Agent profiles:\n");
        for p in &self.profiles {
            let marker = if self.default_profile.as_deref() == Some(p.as_str()) {
                " (default)"
            } else {
                ""
            };
            let _ = writeln!(out, "  {p}{marker}");
        }
        out
    }
}

fn list_action(output_format: &str) -> i32 {
    match agent_profiles::collect_status() {
        Ok(report) => print_and_exit(
            ProfileListResult {
                profiles: report.profiles,
                default_profile: report.default_profile,
            },
            output_format,
        ),
        Err(e) => fail("list", "load", &e),
    }
}

/// Newtype so the `Outputable` impl lives with the CLI (WP-B) while the
/// report struct stays WP-A-owned. `serde(transparent)` keeps the JSON
/// identical to serializing the report directly.
#[derive(Serialize)]
#[serde(transparent)]
struct StatusOutput(ProfileStatusReport);

impl Outputable for StatusOutput {
    fn to_human(&self) -> String {
        let mut out = String::new();
        out.push_str("Agent Profile Status\n");
        out.push_str("====================\n");
        let _ = writeln!(
            out,
            "Default profile: {}",
            self.0.default_profile.as_deref().unwrap_or("(none)")
        );
        let _ = writeln!(
            out,
            "Profiles:        {}",
            if self.0.profiles.is_empty() {
                "(none)".to_string()
            } else {
                self.0.profiles.join(", ")
            }
        );
        if !self.0.agents.is_empty() {
            out.push_str("\nAgents:\n");
            for agent in &self.0.agents {
                let active = agent.active_profile.as_deref().unwrap_or("-");
                let _ = writeln!(
                    out,
                    "  agent={}  mechanism={}  switchable={}  active_profile={}  state={}",
                    agent.agent, agent.mechanism, agent.switchable, active, agent.state,
                );
            }
        }
        out
    }
}

fn status_action(output_format: &str) -> i32 {
    match agent_profiles::collect_status() {
        Ok(report) => print_and_exit(StatusOutput(report), output_format),
        Err(e) => fail("status", "report", &e),
    }
}

// ---------------------------------------------------------------------------
// delete

fn delete_action(name: &str) -> i32 {
    match agent_profiles::delete_profile(name) {
        Ok(agent_count) => {
            println!("Deleted profile '{name}'.");
            emit_deleted(agent_count);
            0
        }
        Err(e @ ProfileError::ActiveProfileDelete(_)) => {
            let code = fail("delete", "delete", &e);
            eprintln!(
                "Switch the affected agent(s) first: `jarvy agents profile use <other> --global`."
            );
            code
        }
        Err(e) => fail("delete", "delete", &e),
    }
}

// ---------------------------------------------------------------------------
// save

fn save_action(name: Option<&str>, agents_raw: Option<&str>) -> i32 {
    let started = Instant::now();
    let explicit = agents_raw.is_some();
    let requested: Vec<Agent> = match parse_optional_agents(agents_raw) {
        Ok(Some(list)) => list,
        Ok(None) => Agent::ALL.to_vec(),
        Err(code) => return code,
    };

    // Every agent is now switchable — filter kept as a defensive nod to
    // future non-switchable variants (`profile_switchable` is not `const`).
    let targets: Vec<Agent> = requested
        .into_iter()
        .filter(|a| a.profile_switchable())
        .collect();
    if targets.is_empty() {
        eprintln!(
            "Error: no switchable agents selected (agents: {})",
            slug_list(Agent::ALL)
        );
        return CONFIG_ERROR;
    }

    // Load registry ONCE so per-agent active lookups don't re-read.
    // Same for the store root — both are needed by every
    // `resolve_active_profile` call below (hoisted out of the loop).
    let registry = match ProfileRegistry::load() {
        Ok(r) => r,
        Err(e) => return fail("save", "registry_load", &e),
    };
    let store_root = match crate::paths::agent_profiles_dir() {
        Ok(p) => p,
        Err(_) => return fail("save", "registry_load", &ProfileError::NoHome),
    };

    // Split counters: env-tier writes vs symlink-tier no-ops vs skipped
    // (no active profile for that agent). Collapsing them into one
    // integer hid "half the agents were skipped for reasons" behind
    // "everything ran cleanly" — a `save` that touched nothing looked
    // identical to one that fully succeeded.
    let mut env_copied: usize = 0;
    let mut symlink_noop: usize = 0;
    let mut skipped_count: usize = 0;
    for agent in &targets {
        let profile_for_agent: String = match name {
            Some(n) => n.to_string(),
            None => match agent_profiles::resolve_active_profile(*agent, &registry, &store_root) {
                Some(p) => p,
                None => {
                    eprintln!(
                        "note: {} has no active profile — skipping (run `jarvy agents profile use <name>` first, or pass a profile name)",
                        agent.slug()
                    );
                    skipped_count += 1;
                    continue;
                }
            },
        };

        match agent_profiles::save_agent(*agent, &profile_for_agent) {
            Ok(SaveOutcome::EnvCopied) => {
                println!("Saved {} → '{}'.", agent.slug(), profile_for_agent);
                env_copied += 1;
            }
            Ok(SaveOutcome::SymlinkNoOp) => {
                println!(
                    "note: {} is symlink-tier and already writes through the live link — nothing to save",
                    agent.slug()
                );
                symlink_noop += 1;
            }
            Err(e) => return fail("save", "snapshot", &e),
        }
    }

    emit_saved(
        env_copied,
        symlink_noop,
        skipped_count,
        explicit,
        started.elapsed().as_millis() as u64,
    );
    0
}

// `active_profile_for_agent` was folded into
// `agent_profiles::resolve_active_profile` — the "env → registry →
// symlink" order was duplicated in `status::check_preference` (with a
// different mechanism-walk shape), and the two would drift the day an
// agent had both Env AND Symlink mechanisms.

// ---------------------------------------------------------------------------
// restore

fn restore_action(name: &str, agents_raw: Option<&str>, force: bool) -> i32 {
    let started = Instant::now();
    let explicit = agents_raw.is_some();
    let requested: Vec<Agent> = match parse_optional_agents(agents_raw) {
        Ok(Some(list)) => list,
        Ok(None) => Agent::ALL.to_vec(),
        Err(code) => return code,
    };

    let targets: Vec<Agent> = requested
        .into_iter()
        .filter(|a| a.profile_switchable())
        .collect();
    if targets.is_empty() {
        eprintln!(
            "Error: no switchable agents selected (agents: {})",
            slug_list(Agent::ALL)
        );
        return CONFIG_ERROR;
    }

    // Verify the profile exists before touching anything.
    let pdir = match agent_profiles::store::profile_dir(name) {
        Ok(p) => p,
        Err(e) => return fail("restore", "load", &e),
    };
    if !pdir.is_dir() {
        let e = ProfileError::ProfileNotFound(name.to_string());
        eprintln!("Error: {e}");
        return fail("restore", "load", &e);
    }

    // Unfiltered restore: skip agents lacking a snapshot in this
    // profile — mirrors the `use` change. Explicit `--agents` still
    // errors on a missing snapshot (the user asked for it, silence
    // would hide the mistake).
    let planned_targets: Vec<Agent> = if explicit {
        targets.clone()
    } else {
        targets
            .iter()
            .copied()
            .filter(|a| pdir.join(a.slug()).is_dir())
            .collect()
    };
    // Track unfiltered agents that got dropped (no snapshot) so
    // `skipped_count` reflects them, even though the CLI prints
    // nothing for the silent skip.
    let mut skipped_count: usize = targets.len().saturating_sub(planned_targets.len());

    let mut env_copied: usize = 0;
    let mut symlink_repointed: usize = 0;
    let mut restored: Vec<Agent> = Vec::new();
    for agent in &planned_targets {
        // Running-IDE guard on symlink-tier targets — mirrors
        // `use --global`. Fail-safe: `is_ide_running` returns false on
        // any probe failure, so the guard only fires on a positive
        // detection. `--force` is the documented escape hatch.
        if agent.is_symlink_tier() && agent_profiles::probe::is_ide_running(*agent) {
            if !force {
                emit_ide_running(*agent, false);
                eprintln!(
                    "note: {} is running — refusing to restore. \
                     Quit the editor and rerun, or pass --force.",
                    agent.slug()
                );
                skipped_count += 1;
                continue;
            }
            emit_ide_running(*agent, true);
            eprintln!(
                "warn: {} is running — restoring under --force. \
                 Restart the editor to pick up the restored profile.",
                agent.slug()
            );
        }
        if let Err(e) = agent_profiles::restore_agent(*agent, name) {
            return fail("restore", "snapshot", &e);
        }
        println!("Restored {} from '{}'.", agent.slug(), name);
        if agent.is_symlink_tier() {
            symlink_repointed += 1;
        } else {
            env_copied += 1;
        }
        restored.push(*agent);
    }

    // Update the registry's active map for each restored agent — this is
    // the same shape `use --global` writes. Save happens after the disk
    // work lands, so a save failure still leaves the correct on-disk
    // state (mirrors `use`'s split-window semantics).
    let mut registry = match ProfileRegistry::load() {
        Ok(r) => r,
        Err(e) => return fail("restore", "registry_load", &e),
    };
    for agent in &restored {
        registry
            .active
            .insert(agent.slug().to_string(), name.to_string());
    }
    if let Err(e) = registry.save() {
        return fail("restore", "registry_save", &e);
    }

    emit_restored(
        env_copied,
        symlink_repointed,
        skipped_count,
        explicit,
        started.elapsed().as_millis() as u64,
    );
    0
}

// ---------------------------------------------------------------------------
// shared helpers

/// Maps a `ProfileError` to a bounded telemetry-safe kind string.
/// All variant names are lowercase-snake to keep cardinality bounded.
fn error_kind(e: &ProfileError) -> &'static str {
    match e {
        ProfileError::NoHome => "no_home",
        ProfileError::Io(_) => "io",
        ProfileError::Json(_) => "json",
        ProfileError::InvalidName(_) => "invalid_name",
        ProfileError::UnmanagedDir(_) => "unmanaged_dir",
        ProfileError::ProfileNotFound(_) => "profile_not_found",
        ProfileError::ProfileExists(_) => "profile_exists",
        ProfileError::AgentNotSnapshotted { .. } => "agent_not_snapshotted",
        ProfileError::ActiveProfileDelete(_) => "active_profile_delete",
        ProfileError::EscapesHome(_) => "escapes_home",
    }
}

/// Every `ProfileError` maps to CONFIG_ERROR (2) — parity with
/// `skills_cmd`, which uses the same code for its IO failures.
///
/// `action` is the subcommand that failed and `stage` the step within it.
/// Without them an `error_kind = "io"` row is unattributable: a failed
/// snapshot copy during `init` and a failed registry write during `use`
/// are the same string, and only one of them leaves the store in a
/// half-applied state. `stage` is what tells them apart — a `use` that
/// died at `registry_save` has already re-pointed the live symlink.
fn fail(action: &'static str, stage: &'static str, e: &ProfileError) -> i32 {
    if crate::observability::telemetry_gate::is_enabled() {
        tracing::warn!(
            event = "agent_profile.op_failed",
            action,
            stage,
            error_kind = error_kind(e),
            "agent profile operation failed"
        );
    }
    eprintln!("Error: {e}");
    CONFIG_ERROR
}

fn parse_optional_agents(raw: Option<&str>) -> Result<Option<Vec<Agent>>, i32> {
    match raw {
        None => Ok(None),
        Some(list) => match parse_agents(list) {
            Ok(v) => Ok(Some(v)),
            Err(msg) => {
                eprintln!("Error: {msg}");
                Err(CONFIG_ERROR)
            }
        },
    }
}

fn parse_agents(raw: &str) -> Result<Vec<Agent>, String> {
    let mut out: Vec<Agent> = Vec::new();
    for part in raw.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        match Agent::from_slug(part) {
            Some(a) => {
                if !out.contains(&a) {
                    out.push(a);
                }
            }
            None => {
                return Err(format!(
                    "unknown agent `{part}` — valid agents: {}",
                    slug_list(Agent::ALL)
                ));
            }
        }
    }
    if out.is_empty() {
        Err(format!(
            "empty --agents list — valid agents: {}",
            slug_list(Agent::ALL)
        ))
    } else {
        Ok(out)
    }
}

fn slug_list(agents: &[Agent]) -> String {
    agents
        .iter()
        .map(|a| a.slug())
        .collect::<Vec<_>>()
        .join(", ")
}

fn tier_label(agent: Agent) -> &'static str {
    if agent.is_symlink_tier() {
        "symlink tier — switches globally"
    } else {
        "env tier — per-terminal"
    }
}

// ---------------------------------------------------------------------------
// telemetry (gated; profile names NEVER emitted)

fn emit_created(
    action: &'static str,
    seeded_from_existing: bool,
    agents_filtered: bool,
    duration_ms: u64,
    agent_count: usize,
) {
    if crate::observability::telemetry_gate::is_enabled() {
        tracing::info!(
            event = "agent_profile.created",
            action,
            seeded_from_existing,
            agents_filtered,
            duration_ms,
            agent_count,
        );
    }
}

fn emit_deleted(agent_count: usize) {
    if crate::observability::telemetry_gate::is_enabled() {
        tracing::info!(event = "agent_profile.deleted", agent_count);
    }
}

/// `agent_count` is retained as the SUM of `env_copied + symlink_noop`
/// (skipped agents don't contribute — the whole point of `skipped_count`
/// is that they didn't run). Kept for backwards compat with dashboards
/// that already query it; the breakdown fields (`env_copied`,
/// `symlink_noop`, `skipped_count`) are the new signal — see the
/// `skills.updated` shape in CLAUDE.md for the pattern.
fn emit_saved(
    env_copied: usize,
    symlink_noop: usize,
    skipped_count: usize,
    agents_filtered: bool,
    duration_ms: u64,
) {
    if crate::observability::telemetry_gate::is_enabled() {
        let agent_count = env_copied + symlink_noop;
        tracing::info!(
            event = "agent_profile.saved",
            agent_count,
            env_copied,
            symlink_noop,
            skipped_count,
            agents_filtered,
            duration_ms,
        );
    }
}

/// Same shape as [`emit_saved`]: `agent_count = env_copied + symlink_repointed`
/// for taxonomy compat, plus the breakdown fields so a run that skipped
/// half the agents (no snapshot) is distinguishable from one that
/// restored everything.
fn emit_restored(
    env_copied: usize,
    symlink_repointed: usize,
    skipped_count: usize,
    agents_filtered: bool,
    duration_ms: u64,
) {
    if crate::observability::telemetry_gate::is_enabled() {
        let agent_count = env_copied + symlink_repointed;
        tracing::info!(
            event = "agent_profile.restored",
            agent_count,
            env_copied,
            symlink_repointed,
            skipped_count,
            agents_filtered,
            duration_ms,
        );
    }
}

/// One `agent_profile.switched` event PER AGENT so the `mechanism`
/// label stays bounded ("env" | "symlink") and per-agent attribution
/// is preserved. `invocation_source` distinguishes the `jp` shell
/// shorthand from a direct CLI call.
fn emit_switched_env(
    targets: &[Agent],
    global: bool,
    agents_filtered: bool,
    duration_ms: u64,
    pre_active: &std::collections::BTreeMap<String, String>,
) {
    let scope = if global { "global" } else { "tty" };
    for agent in targets {
        if agent.is_symlink_tier() {
            continue;
        }
        emit_switched_one(
            *agent,
            "env",
            scope,
            agents_filtered,
            duration_ms,
            pre_active,
        );
    }
}

fn emit_switched_one(
    agent: Agent,
    mechanism: &'static str,
    scope: &'static str,
    agents_filtered: bool,
    duration_ms: u64,
    pre_active: &std::collections::BTreeMap<String, String>,
) {
    if !crate::observability::telemetry_gate::is_enabled() {
        return;
    }
    // Same vocabulary as the `ensure.*` family: `jp` is the shell-rc
    // snippet, so the two domains join on this field instead of each
    // spelling "came from the rc snippet" its own way.
    let invocation_source =
        if std::env::var_os("JARVY_JP_INVOCATION").is_some_and(|v| !v.is_empty()) {
            "rc_snippet"
        } else {
            "manual"
        };

    tracing::info!(
        event = "agent_profile.switched",
        agent = agent.slug(),
        mechanism,
        scope,
        duration_ms,
        invocation_source,
        first_switch = !pre_active.contains_key(agent.slug()),
        agents_filtered,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_profiles::test_support::{JarvyHomeGuard, seed_snapshot};

    fn profile_cmd(action: ProfileAction) -> AgentsAction {
        AgentsAction::Profile { action }
    }

    #[test]
    fn format_exports_emits_bash_lines_and_escapes() {
        let out = format_exports(&[
            (
                "CLAUDE_CONFIG_DIR",
                PathBuf::from("/home/u/.jarvy/agent-profiles/work/claude-code"),
            ),
            ("CODEX_HOME", PathBuf::from("/tmp/we\"ird$pa`th\\x")),
        ])
        .unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(
            lines[0],
            "export CLAUDE_CONFIG_DIR=\"/home/u/.jarvy/agent-profiles/work/claude-code\""
        );
        assert_eq!(
            lines[1],
            "export CODEX_HOME=\"/tmp/we\\\"ird\\$pa\\`th\\\\x\""
        );
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn format_exports_empty_is_empty() {
        assert!(format_exports(&[]).unwrap().is_empty());
    }

    #[test]
    fn format_exports_refuses_control_bytes() {
        // A newline in the path would split the export line — must be refused.
        let path_with_newline = PathBuf::from("/home/u/bad\npath");
        let result = format_exports(&[("CLAUDE_CONFIG_DIR", path_with_newline)]);
        assert!(
            result.is_err(),
            "path with embedded newline must be refused"
        );

        // Carriage return is also a control byte.
        let path_with_cr = PathBuf::from("/home/u/bad\rpath");
        let result = format_exports(&[("CODEX_HOME", path_with_cr)]);
        assert!(
            result.is_err(),
            "path with embedded carriage return must be refused"
        );
    }

    #[test]
    fn parse_agents_accepts_slugs_case_insensitive_and_dedupes() {
        let agents = parse_agents("claude-code, Codex,claude-code").unwrap();
        assert_eq!(agents, vec![Agent::ClaudeCode, Agent::Codex]);
    }

    #[test]
    fn parse_agents_rejects_unknown_with_valid_list() {
        let err = parse_agents("claude-code,nope").unwrap_err();
        assert!(err.contains("nope"));
        assert!(err.contains("claude-code"));
        assert!(err.contains("cursor"));
    }

    #[test]
    fn parse_agents_rejects_empty() {
        assert!(parse_agents("").is_err());
        assert!(parse_agents(" , ").is_err());
    }

    #[test]
    fn list_result_renders_default_marker_and_json() {
        let result = ProfileListResult {
            profiles: vec!["default".into(), "work".into()],
            default_profile: Some("work".into()),
        };
        let human = result.to_human();
        assert!(human.contains("  default\n"));
        assert!(human.contains("  work (default)\n"));
        let json = result.to_json();
        assert!(json.contains("\"work\""));
        assert!(json.contains("\"default_profile\""));
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn init_on_empty_home_creates_default_profile() {
        let _home = JarvyHomeGuard::new();
        let code = run_agents_profile(&profile_cmd(ProfileAction::Init { agents: None }));
        assert_eq!(code, 0);

        assert!(agent_profiles::profile_dir("default").unwrap().is_dir());
        let registry = ProfileRegistry::load().unwrap();
        assert_eq!(registry.default_profile.as_deref(), Some("default"));

        // Idempotent re-run.
        assert_eq!(
            run_agents_profile(&profile_cmd(ProfileAction::Init { agents: None })),
            0
        );
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn create_shows_up_in_list_json_and_refuses_dup() {
        let _home = JarvyHomeGuard::new();
        let code = run_agents_profile(&profile_cmd(ProfileAction::Create {
            name: "work".into(),
            from: None,
            agents: None,
        }));
        assert_eq!(code, 0);

        let report = agent_profiles::collect_status().unwrap();
        let json = ProfileListResult {
            profiles: report.profiles,
            default_profile: report.default_profile,
        }
        .to_json();
        assert!(json.contains("\"work\""));

        let dup = run_agents_profile(&profile_cmd(ProfileAction::Create {
            name: "work".into(),
            from: None,
            agents: None,
        }));
        assert_eq!(dup, CONFIG_ERROR);
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn create_with_unknown_agent_slug_is_config_error() {
        let _home = JarvyHomeGuard::new();
        let code = run_agents_profile(&profile_cmd(ProfileAction::Create {
            name: "narrow".into(),
            from: None,
            agents: Some("claude-code,bogus".into()),
        }));
        assert_eq!(code, CONFIG_ERROR);
        // Nothing created on refusal.
        assert!(!agent_profiles::profile_dir("narrow").unwrap().exists());
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn delete_removes_profile_and_second_delete_errors() {
        let _home = JarvyHomeGuard::new();
        assert_eq!(
            run_agents_profile(&profile_cmd(ProfileAction::Create {
                name: "gone".into(),
                from: None,
                agents: None,
            })),
            0
        );
        assert_eq!(
            run_agents_profile(&profile_cmd(ProfileAction::Delete {
                name: "gone".into()
            })),
            0
        );
        assert_eq!(
            run_agents_profile(&profile_cmd(ProfileAction::Delete {
                name: "gone".into()
            })),
            CONFIG_ERROR
        );
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn use_unknown_profile_is_config_error() {
        let _home = JarvyHomeGuard::new();
        let code = run_agents_profile(&profile_cmd(ProfileAction::Use {
            name: "missing".into(),
            agents: Some("claude-code".into()),
            global: false,
            print_env: true,
            force: false,
        }));
        assert_eq!(code, CONFIG_ERROR);
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn use_env_tier_exports_carry_profile_path_and_handler_exits_zero() {
        let _home = JarvyHomeGuard::new();
        agent_profiles::create_profile("work", None).unwrap();
        let snap = seed_snapshot("work", Agent::ClaudeCode);

        // Composition check: the plan's exports formatted through
        // format_exports carry the snapshot path under the var the
        // shell snippet evals.
        let plans = agent_profiles::plan_use("work", &[Agent::ClaudeCode], false).unwrap();
        let exports = match &plans[0] {
            SwitchPlan::EnvExports(e) => e.clone(),
            other => panic!("expected EnvExports, got {other:?}"),
        };
        let block = format_exports(&exports).unwrap();
        assert!(block.starts_with("export CLAUDE_CONFIG_DIR=\""));
        // Windows paths contain `\` which `format_exports` escapes to `\\`,
        // so we can't do a direct substring match on `snap.display()`.
        // Assert the profile name + agent slug appear instead — both survive
        // escaping and prove the export line points at the right snapshot.
        assert!(block.contains("work"));
        assert!(block.contains("claude-code"));
        let _ = snap;

        // Handler-level: same selection succeeds end-to-end.
        let code = run_agents_profile(&profile_cmd(ProfileAction::Use {
            name: "work".into(),
            agents: Some("claude-code".into()),
            global: false,
            print_env: true,
            force: false,
        }));
        assert_eq!(code, 0);
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn use_cursor_only_without_global_exits_zero() {
        let _home = JarvyHomeGuard::new();
        agent_profiles::create_profile("work", None).unwrap();
        seed_snapshot("work", Agent::Cursor);

        // Targets are non-empty (cursor IS v1-switchable) but the plan
        // is empty without --global: "No env-tier exports" + a notice,
        // exit 0 — NOT a config error.
        let code = run_agents_profile(&profile_cmd(ProfileAction::Use {
            name: "work".into(),
            agents: Some("cursor".into()),
            global: false,
            print_env: false,
            force: false,
        }));
        assert_eq!(code, 0);
        // Nothing was repointed (no --global): no live symlink.
        assert!(std::fs::symlink_metadata(Agent::Cursor.config_dir().unwrap()).is_err());
    }

    #[cfg(unix)]
    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn delete_active_symlink_profile_via_handler_is_config_error() {
        let _home = JarvyHomeGuard::new();
        // Seed a live cursor dir, init so it's moved + linked back.
        std::fs::create_dir_all(Agent::Cursor.config_dir().unwrap()).unwrap();
        assert_eq!(
            run_agents_profile(&profile_cmd(ProfileAction::Init { agents: None })),
            0
        );

        let code = run_agents_profile(&profile_cmd(ProfileAction::Delete {
            name: "default".into(),
        }));
        assert_eq!(code, CONFIG_ERROR);
        // Profile survives the refusal.
        assert!(agent_profiles::profile_dir("default").unwrap().is_dir());
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn create_from_missing_source_is_config_error() {
        let _home = JarvyHomeGuard::new();
        let code = run_agents_profile(&profile_cmd(ProfileAction::Create {
            name: "new".into(),
            from: Some("missing".into()),
            agents: None,
        }));
        assert_eq!(code, CONFIG_ERROR);
        // Nothing left behind for the refused create.
        assert!(!agent_profiles::profile_dir("new").unwrap().exists());
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn create_agents_narrowing_prunes_snapshots() {
        let _home = JarvyHomeGuard::new();
        agent_profiles::create_profile("src", None).unwrap();
        seed_snapshot("src", Agent::ClaudeCode);
        seed_snapshot("src", Agent::Codex);

        let code = run_agents_profile(&profile_cmd(ProfileAction::Create {
            name: "narrow".into(),
            from: Some("src".into()),
            agents: Some("claude-code".into()),
        }));
        assert_eq!(code, 0);

        let narrow = agent_profiles::profile_dir("narrow").unwrap();
        assert!(narrow.join("claude-code").is_dir(), "kept agent survives");
        assert!(!narrow.join("codex").exists(), "excluded agent pruned");
        // Source profile untouched by the narrowing.
        let src = agent_profiles::profile_dir("src").unwrap();
        assert!(src.join("claude-code").is_dir());
        assert!(src.join("codex").is_dir());
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn status_json_shape_is_stable() {
        let _home = JarvyHomeGuard::new();
        agent_profiles::create_profile("work", None).unwrap();
        let report = agent_profiles::collect_status().unwrap();
        let json = StatusOutput(report).to_json();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();

        let obj = value.as_object().expect("top level is an object");
        for key in ["profiles", "default_profile", "agents"] {
            assert!(obj.contains_key(key), "missing top-level key {key}");
        }
        let agents = value["agents"].as_array().expect("agents is an array");
        assert_eq!(agents.len(), Agent::COUNT);
        let entry = agents[0].as_object().expect("agent entry is an object");
        for key in [
            "agent",
            "mechanism",
            "switchable",
            "active_profile",
            "state",
        ] {
            assert!(entry.contains_key(key), "missing agent key {key}");
        }
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn init_twice_env_tier_refreshes_snapshot_from_live() {
        // Documents actual handler-level idempotency: a second init
        // re-COPIES the live env-tier dir over the snapshot (save
        // semantics). Unchanged live == unchanged snapshot; a changed
        // live dir is re-captured. Exit 0 both times.
        let _home = JarvyHomeGuard::new();
        let live = Agent::ClaudeCode.config_dir().unwrap();
        std::fs::create_dir_all(&live).unwrap();
        std::fs::write(live.join("settings.json"), "v1").unwrap();
        assert_eq!(
            run_agents_profile(&profile_cmd(ProfileAction::Init { agents: None })),
            0
        );

        let snap = agent_profiles::profile_dir("default")
            .unwrap()
            .join("claude-code")
            .join("settings.json");
        assert_eq!(std::fs::read_to_string(&snap).unwrap(), "v1");

        // Unchanged live: second init leaves the snapshot content intact.
        assert_eq!(
            run_agents_profile(&profile_cmd(ProfileAction::Init { agents: None })),
            0
        );
        assert_eq!(std::fs::read_to_string(&snap).unwrap(), "v1");
        // Live dir itself untouched (still a real dir with content).
        assert_eq!(
            std::fs::read_to_string(live.join("settings.json")).unwrap(),
            "v1"
        );

        // Changed live: a third init re-captures the new content.
        std::fs::write(live.join("settings.json"), "v2").unwrap();
        assert_eq!(
            run_agents_profile(&profile_cmd(ProfileAction::Init { agents: None })),
            0
        );
        assert_eq!(std::fs::read_to_string(&snap).unwrap(), "v2");
    }

    /// `error_kind` covers every `ProfileError` variant — this test
    /// acts as an exhaustiveness guard: adding a variant without updating
    /// the match will cause a compile error, not a test failure.
    #[test]
    fn error_kind_mapping_is_exhaustive() {
        let cases: &[ProfileError] = &[
            ProfileError::NoHome,
            ProfileError::Io(std::io::Error::other("x")),
            ProfileError::Json(serde_json::from_str::<serde_json::Value>("!").unwrap_err()),
            ProfileError::InvalidName("x".into()),
            ProfileError::UnmanagedDir(PathBuf::from("/x")),
            ProfileError::ProfileNotFound("x".into()),
            ProfileError::ProfileExists("x".into()),
            ProfileError::AgentNotSnapshotted {
                agent: "x".into(),
                profile: "y".into(),
            },
            ProfileError::ActiveProfileDelete("x".into()),
            ProfileError::EscapesHome(PathBuf::from("/x")),
        ];
        for e in cases {
            // Just assert it returns a non-empty string — the
            // compile-time exhaustiveness check is the real guard.
            assert!(!error_kind(e).is_empty());
        }
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn use_with_explicit_agents_does_not_update_default_profile() {
        let _home = JarvyHomeGuard::new();
        // Seed both profiles and agent snapshots.
        agent_profiles::create_profile("work", None).unwrap();
        agent_profiles::create_profile("other", None).unwrap();
        seed_snapshot("work", Agent::ClaudeCode);
        seed_snapshot("work", Agent::Cursor);

        // Set initial default to "other" via registry.
        let mut reg = ProfileRegistry::load().unwrap();
        reg.default_profile = Some("other".to_string());
        reg.save().unwrap();

        // use work --agents claude-code (partial): default must remain "other".
        let code = run_agents_profile(&profile_cmd(ProfileAction::Use {
            name: "work".into(),
            agents: Some("claude-code".into()),
            global: true,
            print_env: false,
            force: false,
        }));
        assert_eq!(code, 0);

        let registry = ProfileRegistry::load().unwrap();
        assert_eq!(
            registry.default_profile.as_deref(),
            Some("other"),
            "default profile must not be updated on partial --agents switch"
        );
        // But the targeted agent's active entry IS updated.
        assert_eq!(
            registry.active.get("claude-code").map(String::as_str),
            Some("work")
        );
        // Cursor was not targeted — its registry entry must be unchanged.
        assert!(
            registry.active.get("cursor").map(String::as_str) != Some("work"),
            "cursor was not targeted and must not be updated"
        );
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn init_with_explicit_agents_does_not_clobber_existing_default() {
        let _home = JarvyHomeGuard::new();
        agent_profiles::create_profile("other", None).unwrap();
        let mut reg = ProfileRegistry::load().unwrap();
        reg.default_profile = Some("other".to_string());
        reg.save().unwrap();

        let live = Agent::ClaudeCode.config_dir().unwrap();
        std::fs::create_dir_all(&live).unwrap();
        std::fs::write(live.join("settings.json"), "v1").unwrap();

        assert_eq!(
            run_agents_profile(&profile_cmd(ProfileAction::Init {
                agents: Some("claude-code".into()),
            })),
            0
        );

        let reg = ProfileRegistry::load().unwrap();
        assert_eq!(
            reg.default_profile.as_deref(),
            Some("other"),
            "a narrowed init must not repoint the default profile"
        );
        // The named agent was still snapshotted and marked active.
        assert_eq!(
            reg.active.get("claude-code").map(String::as_str),
            Some("default")
        );
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn init_with_explicit_agents_claims_default_when_unset() {
        // Narrowing only defers to an EXISTING default. On a virgin store
        // there is nothing to protect, and refusing to set one would leave
        // `use` with no default to fall back on.
        let _home = JarvyHomeGuard::new();
        assert_eq!(
            run_agents_profile(&profile_cmd(ProfileAction::Init {
                agents: Some("claude-code".into()),
            })),
            0
        );
        assert_eq!(
            ProfileRegistry::load().unwrap().default_profile.as_deref(),
            Some("default")
        );
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn init_with_explicit_agents_skips_unlisted_agents() {
        let _home = JarvyHomeGuard::new();
        for agent in [Agent::ClaudeCode, Agent::Codex] {
            let live = agent.config_dir().unwrap();
            std::fs::create_dir_all(&live).unwrap();
            std::fs::write(live.join("marker"), "x").unwrap();
        }
        assert_eq!(
            run_agents_profile(&profile_cmd(ProfileAction::Init {
                agents: Some("codex".into()),
            })),
            0
        );
        let pdir = agent_profiles::profile_dir("default").unwrap();
        assert!(pdir.join("codex").is_dir(), "named agent snapshotted");
        assert!(
            !pdir.join("claude-code").exists(),
            "unnamed agent must be skipped entirely"
        );
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn init_with_unknown_agent_slug_is_config_error() {
        let _home = JarvyHomeGuard::new();
        assert_eq!(
            run_agents_profile(&profile_cmd(ProfileAction::Init {
                agents: Some("claude-code,bogus".into()),
            })),
            CONFIG_ERROR
        );
        // Refused before any store mutation.
        assert!(!agent_profiles::profile_dir("default").unwrap().exists());
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn status_human_output_contains_expected_labels() {
        let _home = JarvyHomeGuard::new();
        agent_profiles::create_profile("work", None).unwrap();
        let report = agent_profiles::collect_status().unwrap();
        let output = StatusOutput(report).to_human();
        assert!(output.contains("claude-code"), "missing agent slug");
        assert!(output.contains("env"), "missing mechanism label");
        assert!(
            output.contains("managed") || output.contains("not_installed"),
            "missing state"
        );
        assert!(output.contains("switchable"), "missing field label");
    }

    #[cfg(unix)]
    #[test]
    #[serial_test::serial(jarvy_home_env, probe_env)]
    fn init_after_use_global_reconciles_registry() {
        // init → seed work → use work --global → init again →
        // registry.active["cursor"] must reflect the link target ("work",
        // not "default" from the first init).
        // Probe env guard: `use --global` on Cursor runs the running-IDE
        // check; without JARVY_TEST_MODE=1 a Cursor on the dev box would
        // refuse the swap and turn this into a flake.
        let _probe = ProbeEnvGuard::new();
        let _home = JarvyHomeGuard::new();
        // Seed cursor live dir so init can snapshot it.
        std::fs::create_dir_all(Agent::Cursor.config_dir().unwrap()).unwrap();
        assert_eq!(
            run_agents_profile(&profile_cmd(ProfileAction::Init { agents: None })),
            0
        );

        // Create work + seed cursor snapshot.
        agent_profiles::create_profile("work", None).unwrap();
        seed_snapshot("work", Agent::Cursor);

        // use work --global switches the cursor symlink. Narrow to cursor:
        // an unfiltered use would refuse because work has no claude-code /
        // codex snapshots (matches the e2e's pre-seed requirement).
        let code = run_agents_profile(&profile_cmd(ProfileAction::Use {
            name: "work".into(),
            agents: Some("cursor".into()),
            global: true,
            print_env: false,
            force: false,
        }));
        assert_eq!(code, 0);

        // A second init should reconcile the registry from the live link.
        assert_eq!(
            run_agents_profile(&profile_cmd(ProfileAction::Init { agents: None })),
            0
        );

        let registry = ProfileRegistry::load().unwrap();
        assert_eq!(
            registry.active.get("cursor").map(String::as_str),
            Some("work"),
            "after re-init, cursor registry entry must match the live symlink target"
        );
    }

    // ── save / restore CLI tests ───────────────────────────────────────

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn save_env_tier_refreshes_snapshot_from_live() {
        let _home = JarvyHomeGuard::new();
        agent_profiles::create_profile("work", None).unwrap();
        let live = Agent::ClaudeCode.config_dir().unwrap();
        std::fs::create_dir_all(&live).unwrap();
        std::fs::write(live.join("settings.json"), "live-v1").unwrap();

        let code = run_agents_profile(&profile_cmd(ProfileAction::Save {
            name: Some("work".into()),
            agents: Some("claude-code".into()),
        }));
        assert_eq!(code, 0);

        let snap = agent_profiles::profile_dir("work")
            .unwrap()
            .join("claude-code")
            .join("settings.json");
        assert_eq!(std::fs::read_to_string(&snap).unwrap(), "live-v1");

        // Mutate live, save again — snapshot mirrors.
        std::fs::write(live.join("settings.json"), "live-v2").unwrap();
        let code = run_agents_profile(&profile_cmd(ProfileAction::Save {
            name: Some("work".into()),
            agents: Some("claude-code".into()),
        }));
        assert_eq!(code, 0);
        assert_eq!(std::fs::read_to_string(&snap).unwrap(), "live-v2");
    }

    #[cfg(unix)]
    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn save_symlink_tier_is_noop_when_managed() {
        let _home = JarvyHomeGuard::new();
        std::fs::create_dir_all(Agent::Cursor.config_dir().unwrap()).unwrap();
        assert_eq!(
            run_agents_profile(&profile_cmd(ProfileAction::Init { agents: None })),
            0
        );
        // No error, exit 0 — the symlink-tier live path already writes
        // into the profile snapshot.
        let code = run_agents_profile(&profile_cmd(ProfileAction::Save {
            name: Some("default".into()),
            agents: Some("cursor".into()),
        }));
        assert_eq!(code, 0);
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn save_symlink_tier_reports_unmanaged() {
        let _home = JarvyHomeGuard::new();
        agent_profiles::create_profile("work", None).unwrap();
        std::fs::create_dir_all(Agent::Cursor.config_dir().unwrap()).unwrap();
        let code = run_agents_profile(&profile_cmd(ProfileAction::Save {
            name: Some("work".into()),
            agents: Some("cursor".into()),
        }));
        assert_eq!(code, CONFIG_ERROR);
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn save_default_uses_active_profile_per_agent() {
        let _home = JarvyHomeGuard::new();
        // Two profiles; claude-code registry-active for 'work'.
        agent_profiles::create_profile("work", None).unwrap();
        agent_profiles::create_profile("other", None).unwrap();
        let mut reg = ProfileRegistry::load().unwrap();
        reg.active
            .insert("claude-code".to_string(), "work".to_string());
        reg.save().unwrap();

        let live = Agent::ClaudeCode.config_dir().unwrap();
        std::fs::create_dir_all(&live).unwrap();
        std::fs::write(live.join("settings.json"), "current").unwrap();

        // Save without a name: 'work' is the active profile.
        let code = run_agents_profile(&profile_cmd(ProfileAction::Save {
            name: None,
            agents: Some("claude-code".into()),
        }));
        assert_eq!(code, 0);
        let snap = agent_profiles::profile_dir("work")
            .unwrap()
            .join("claude-code")
            .join("settings.json");
        assert_eq!(std::fs::read_to_string(&snap).unwrap(), "current");
        // 'other' was NOT touched (no snapshot planted there).
        assert!(
            !agent_profiles::profile_dir("other")
                .unwrap()
                .join("claude-code")
                .exists()
        );
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn save_missing_profile_errors() {
        let _home = JarvyHomeGuard::new();
        std::fs::create_dir_all(Agent::ClaudeCode.config_dir().unwrap()).unwrap();
        let code = run_agents_profile(&profile_cmd(ProfileAction::Save {
            name: Some("ghost".into()),
            agents: Some("claude-code".into()),
        }));
        assert_eq!(code, CONFIG_ERROR);
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn restore_env_tier_copies_snapshot_over_live() {
        let _home = JarvyHomeGuard::new();
        agent_profiles::create_profile("work", None).unwrap();
        let snap = seed_snapshot("work", Agent::ClaudeCode);
        std::fs::write(snap.join("settings.json"), "snapshot-v").unwrap();
        // Live has Y (different content).
        let live = Agent::ClaudeCode.config_dir().unwrap();
        std::fs::create_dir_all(&live).unwrap();
        std::fs::write(live.join("stale.txt"), "gone").unwrap();

        let code = run_agents_profile(&profile_cmd(ProfileAction::Restore {
            name: "work".into(),
            agents: Some("claude-code".into()),
            force: false,
        }));
        assert_eq!(code, 0);

        assert_eq!(
            std::fs::read_to_string(live.join("settings.json")).unwrap(),
            "snapshot-v"
        );
        assert!(!live.join("stale.txt").exists());
    }

    #[cfg(unix)]
    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn restore_symlink_tier_repoints_link() {
        let _home = JarvyHomeGuard::new();
        let _probe = ProbeEnvGuard::new();
        agent_profiles::create_profile("a", None).unwrap();
        agent_profiles::create_profile("b", None).unwrap();
        let a = seed_snapshot("a", Agent::Cursor);
        let b = seed_snapshot("b", Agent::Cursor);
        // Start pointed at a.
        let live = Agent::Cursor.config_dir().unwrap();
        std::os::unix::fs::symlink(&a, &live).unwrap();

        let code = run_agents_profile(&profile_cmd(ProfileAction::Restore {
            name: "b".into(),
            agents: Some("cursor".into()),
            force: false,
        }));
        assert_eq!(code, 0);
        assert_eq!(std::fs::read_link(&live).unwrap(), b);
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn restore_missing_snapshot_errors() {
        let _home = JarvyHomeGuard::new();
        agent_profiles::create_profile("work", None).unwrap();
        let code = run_agents_profile(&profile_cmd(ProfileAction::Restore {
            name: "work".into(),
            agents: Some("claude-code".into()),
            force: false,
        }));
        assert_eq!(code, CONFIG_ERROR);
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn restore_updates_registry_active() {
        let _home = JarvyHomeGuard::new();
        agent_profiles::create_profile("work", None).unwrap();
        seed_snapshot("work", Agent::ClaudeCode);
        std::fs::create_dir_all(Agent::ClaudeCode.config_dir().unwrap()).unwrap();

        let code = run_agents_profile(&profile_cmd(ProfileAction::Restore {
            name: "work".into(),
            agents: Some("claude-code".into()),
            force: false,
        }));
        assert_eq!(code, 0);

        let reg = ProfileRegistry::load().unwrap();
        assert_eq!(
            reg.active.get("claude-code").map(String::as_str),
            Some("work")
        );
    }

    #[cfg(unix)]
    #[test]
    #[serial_test::serial(jarvy_home_env, probe_env)]
    fn registry_save_failure_documents_split_state_window() {
        // After init + create work, chmod the registry file to 0o000,
        // then run use work --global. The symlink re-point lands on disk
        // BEFORE the registry save — a save failure leaves split state.
        // This test documents that window and asserts: non-zero return
        // AND the cursor link already points into work.
        // Probe env guard: same reasoning as
        // `init_after_use_global_reconciles_registry` — a live Cursor
        // on the dev box would refuse the swap before the save even
        // ran, breaking the split-state assertion.
        use std::os::unix::fs::PermissionsExt;

        let _probe = ProbeEnvGuard::new();
        let _home = JarvyHomeGuard::new();
        // Set up cursor live dir.
        std::fs::create_dir_all(Agent::Cursor.config_dir().unwrap()).unwrap();
        assert_eq!(
            run_agents_profile(&profile_cmd(ProfileAction::Init { agents: None })),
            0
        );

        // Create and seed work profile.
        agent_profiles::create_profile("work", None).unwrap();
        seed_snapshot("work", Agent::Cursor);

        // Block the registry file from being written.
        let reg_path = crate::agent_profiles::registry_path().unwrap();
        // Ensure file exists first (init wrote it).
        assert!(reg_path.exists());
        std::fs::set_permissions(&reg_path, std::fs::Permissions::from_mode(0o000)).unwrap();

        let code = run_agents_profile(&profile_cmd(ProfileAction::Use {
            name: "work".into(),
            agents: Some("cursor".into()),
            global: true,
            print_env: false,
            force: false,
        }));

        // Restore perms so test cleanup can proceed.
        std::fs::set_permissions(&reg_path, std::fs::Permissions::from_mode(0o600)).unwrap();

        assert_ne!(code, 0, "save failure must return non-zero");

        // The cursor symlink was already re-pointed (documents the window).
        let cursor_link = Agent::Cursor.config_dir().unwrap();
        let target = std::fs::read_link(&cursor_link).unwrap();
        let work_snap = agent_profiles::profile_dir("work").unwrap().join("cursor");
        assert_eq!(
            target, work_snap,
            "cursor link points into work despite registry save failure (documented window)"
        );
    }

    /// RAII guard for `JARVY_MOCK_RUNNING_IDES` — the probe unit tests
    /// set this too, and a panicking test between them would otherwise
    /// leak the mock into an unrelated serial group.
    ///
    /// Every caller of `MockRunningIdesGuard::set` in this file is a
    /// `#[cfg(unix)]` test (the symlink-tier / running-IDE suite is
    /// unix-only here), so this type is unused on non-unix — cfg-gate
    /// it to match rather than leaving it to be flagged dead code.
    #[cfg(unix)]
    struct MockRunningIdesGuard;
    #[cfg(unix)]
    impl MockRunningIdesGuard {
        fn set(value: &str) -> Self {
            #[allow(unsafe_code)]
            unsafe {
                // Probe.rs short-circuits on JARVY_TEST_MODE=1 BEFORE
                // consulting the mock env — clear it here so the mock
                // path is exercised.
                std::env::remove_var("JARVY_TEST_MODE");
                std::env::set_var("JARVY_MOCK_RUNNING_IDES", value);
            }
            Self
        }
    }
    #[cfg(unix)]
    impl Drop for MockRunningIdesGuard {
        fn drop(&mut self) {
            #[allow(unsafe_code)]
            unsafe {
                std::env::remove_var("JARVY_MOCK_RUNNING_IDES");
            }
        }
    }

    /// RAII: pins `JARVY_TEST_MODE=1` for the duration of a test so
    /// `probe::is_ide_running` short-circuits to `false` instead of
    /// spawning `pgrep`/`tasklist` against the developer's live
    /// editors. The probe module no longer has a cfg(test)
    /// short-circuit (the review that removed it added PATH-shim
    /// tests to cover the real primitives) — any in-process CLI test
    /// that performs a symlink-tier `use --global` must opt in
    /// explicitly via this guard, or a running Cursor on the dev box
    /// will refuse the swap. Serialized on the `probe_env` group
    /// alongside `MockRunningIdesGuard` so parallel probe unit tests
    /// can't clobber the env mid-run.
    ///
    /// Same cfg-gating rationale as `MockRunningIdesGuard` above: every
    /// caller is a `#[cfg(unix)]` test.
    #[cfg(unix)]
    struct ProbeEnvGuard;
    #[cfg(unix)]
    impl ProbeEnvGuard {
        fn new() -> Self {
            #[allow(unsafe_code)]
            unsafe {
                std::env::set_var("JARVY_TEST_MODE", "1");
            }
            Self
        }
    }
    #[cfg(unix)]
    impl Drop for ProbeEnvGuard {
        fn drop(&mut self) {
            #[allow(unsafe_code)]
            unsafe {
                std::env::remove_var("JARVY_TEST_MODE");
            }
        }
    }

    /// A running-IDE detection on a symlink-tier swap refuses the
    /// re-point (without `--force`), leaves the live path untouched,
    /// and exits CONFIG_ERROR because nothing else switched either.
    ///
    /// Two serial groups because the test mutates both `JARVY_HOME`
    /// (jarvy_home_env) and the probe's mock env
    /// (probe_env) — otherwise a probe unit test running in parallel
    /// would clobber the mock mid-run.
    #[cfg(unix)]
    #[test]
    #[serial_test::serial(jarvy_home_env, probe_env)]
    fn use_refuses_on_running_ide() {
        let _home = JarvyHomeGuard::new();
        agent_profiles::create_profile("work", None).unwrap();
        seed_snapshot("work", Agent::Cursor);
        // The mock env is a documented test hook — see
        // `probe::is_ide_running`. It short-circuits the platform probe.
        let _mock = MockRunningIdesGuard::set("cursor");

        let code = run_agents_profile(&profile_cmd(ProfileAction::Use {
            name: "work".into(),
            agents: Some("cursor".into()),
            global: true,
            print_env: false,
            force: false,
        }));
        // Nothing else was requested + only symlink target was blocked
        // → whole run bails with CONFIG_ERROR.
        assert_eq!(
            code, CONFIG_ERROR,
            "a running IDE with no other targets must fail the run"
        );
        // No live symlink was created — the refusal fires BEFORE
        // apply_symlink_repoint.
        assert!(
            std::fs::symlink_metadata(Agent::Cursor.config_dir().unwrap()).is_err(),
            "cursor config dir must not exist after refused swap"
        );
        // Registry did not record cursor as active.
        let reg = ProfileRegistry::load().unwrap_or_default();
        assert_ne!(
            reg.active.get("cursor").map(String::as_str),
            Some("work"),
            "registry must not claim a switch that never happened"
        );
    }

    /// `--force` overrides the guard: the re-point lands and the
    /// registry records the switch. Documents the escape-hatch behavior.
    /// Serialized on both groups — see `use_refuses_on_running_ide`.
    #[cfg(unix)]
    #[test]
    #[serial_test::serial(jarvy_home_env, probe_env)]
    fn use_force_bypasses_running_ide_check() {
        let _home = JarvyHomeGuard::new();
        agent_profiles::create_profile("work", None).unwrap();
        let snap = seed_snapshot("work", Agent::Cursor);
        let _mock = MockRunningIdesGuard::set("cursor");

        let code = run_agents_profile(&profile_cmd(ProfileAction::Use {
            name: "work".into(),
            agents: Some("cursor".into()),
            global: true,
            print_env: false,
            force: true,
        }));
        assert_eq!(code, 0, "--force must land the swap");

        // Live symlink now points into the work snapshot.
        let target = std::fs::read_link(Agent::Cursor.config_dir().unwrap()).unwrap();
        assert_eq!(target, snap, "cursor link must be re-pointed under --force");
        // Registry reflects the switch.
        let reg = ProfileRegistry::load().unwrap();
        assert_eq!(
            reg.active.get("cursor").map(String::as_str),
            Some("work"),
            "forced switch must be recorded"
        );
    }

    /// `restore` mirrors `use --global`: a running IDE on the target
    /// symlink-tier agent refuses the re-point. Env-tier restores in
    /// the same run still proceed — the guard is per-agent.
    #[cfg(unix)]
    #[test]
    #[serial_test::serial(jarvy_home_env, probe_env)]
    fn restore_refuses_on_running_ide() {
        let _home = JarvyHomeGuard::new();
        agent_profiles::create_profile("work", None).unwrap();
        seed_snapshot("work", Agent::Cursor);
        let _mock = MockRunningIdesGuard::set("cursor");

        // Only cursor targeted: nothing else can carry the run, but the
        // per-agent guard just skips cursor — the CLI exits 0 with a
        // skipped_count for the blocked agent. The live cursor path stays
        // untouched (no symlink created).
        let code = run_agents_profile(&profile_cmd(ProfileAction::Restore {
            name: "work".into(),
            agents: Some("cursor".into()),
            force: false,
        }));
        assert_eq!(
            code, 0,
            "per-agent skip must not fail the whole run (only cursor was blocked)"
        );
        assert!(
            std::fs::symlink_metadata(Agent::Cursor.config_dir().unwrap()).is_err(),
            "cursor config dir must not exist after refused restore"
        );
        let reg = ProfileRegistry::load().unwrap_or_default();
        assert_ne!(
            reg.active.get("cursor").map(String::as_str),
            Some("work"),
            "registry must not claim a restore that never happened"
        );
    }

    /// `--force` overrides the running-IDE guard on restore, same shape
    /// as `use_force_bypasses_running_ide_check`.
    #[cfg(unix)]
    #[test]
    #[serial_test::serial(jarvy_home_env, probe_env)]
    fn restore_force_bypasses_running_ide_check() {
        let _home = JarvyHomeGuard::new();
        agent_profiles::create_profile("work", None).unwrap();
        let snap = seed_snapshot("work", Agent::Cursor);
        let _mock = MockRunningIdesGuard::set("cursor");

        let code = run_agents_profile(&profile_cmd(ProfileAction::Restore {
            name: "work".into(),
            agents: Some("cursor".into()),
            force: true,
        }));
        assert_eq!(code, 0, "--force must land the restore");
        let target = std::fs::read_link(Agent::Cursor.config_dir().unwrap()).unwrap();
        assert_eq!(target, snap, "cursor link must be re-pointed under --force");
        let reg = ProfileRegistry::load().unwrap();
        assert_eq!(
            reg.active.get("cursor").map(String::as_str),
            Some("work"),
            "forced restore must be recorded"
        );
    }

    /// A restore into an env-tier path (`~/.claude`) that is a live
    /// symlink — user-owned dotfile link, jarvy never creates one there
    /// — must be refused. `--force` does NOT bypass this: overwriting
    /// the user's dotfile link would silently destroy their setup.
    #[cfg(unix)]
    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn restore_env_tier_refuses_over_user_symlink() {
        let _home = JarvyHomeGuard::new();
        agent_profiles::create_profile("work", None).unwrap();
        let snap = seed_snapshot("work", Agent::ClaudeCode);
        std::fs::write(snap.join("settings.json"), "profile-v").unwrap();

        // Simulate a user-owned dotfile link at ~/.claude.
        let dotfiles = crate::agents::home_dir().unwrap().join("dotfiles-claude");
        std::fs::create_dir_all(&dotfiles).unwrap();
        std::fs::write(dotfiles.join("marker"), "mine").unwrap();
        let live = Agent::ClaudeCode.config_dir().unwrap();
        std::os::unix::fs::symlink(&dotfiles, &live).unwrap();

        // The library-level restore must return UnmanagedDir.
        let err = agent_profiles::restore_agent(Agent::ClaudeCode, "work").unwrap_err();
        assert!(
            matches!(err, ProfileError::UnmanagedDir(_)),
            "expected UnmanagedDir refusal, got: {err:?}"
        );

        // Symlink still exists, dotfile marker intact — no destruction.
        let meta = std::fs::symlink_metadata(&live).unwrap();
        assert!(
            meta.file_type().is_symlink(),
            "user symlink at env-tier path must survive the refusal"
        );
        assert_eq!(
            std::fs::read_to_string(dotfiles.join("marker")).unwrap(),
            "mine",
            "user dotfile content must survive the refusal"
        );
    }

    /// `save` for a symlink-tier managed agent counts as `symlink_noop` —
    /// the outcome is `SymlinkNoOp` (already writes through the live
    /// symlink) and the run should exit 0. Together with the `env_copied`
    /// counters, this makes "everything ran cleanly" distinguishable
    /// from "half the agents skipped for reasons" via the emit shape.
    #[cfg(unix)]
    #[test]
    #[serial_test::serial(jarvy_home_env, probe_env)]
    fn save_symlink_noop_counts_correctly() {
        // Init hits `use --global` under the hood for the symlink tier;
        // ProbeEnvGuard keeps a local Cursor from refusing the swap.
        let _probe = ProbeEnvGuard::new();
        let _home = JarvyHomeGuard::new();
        std::fs::create_dir_all(Agent::Cursor.config_dir().unwrap()).unwrap();
        assert_eq!(
            run_agents_profile(&profile_cmd(ProfileAction::Init { agents: None })),
            0
        );

        // Save the managed cursor snapshot — noop, exit 0.
        let code = run_agents_profile(&profile_cmd(ProfileAction::Save {
            name: Some("default".into()),
            agents: Some("cursor".into()),
        }));
        assert_eq!(code, 0, "symlink-tier save must succeed as a no-op");
    }

    /// A bare `save` (no active profile registered anywhere) skips every
    /// agent instead of failing. Exit 0 with `skipped_count` documenting
    /// the outcome via telemetry — the CLI is authoritative-by-count.
    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn save_skipped_no_active_profile_counts_correctly() {
        let _home = JarvyHomeGuard::new();
        // Fresh home: no profiles, no registry entries, no live env-tier
        // dir. `save` walks the agents, finds no active profile per
        // agent, and skips each one. Exit code stays 0 (nothing failed;
        // the run simply had nothing to do).
        let code = run_agents_profile(&profile_cmd(ProfileAction::Save {
            name: None,
            agents: Some("claude-code".into()),
        }));
        assert_eq!(
            code, 0,
            "a `save` with no active profile must skip gracefully, not fail"
        );
    }
}
