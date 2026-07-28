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

use crate::agent_profiles::{self, ProfileError, ProfileRegistry, ProfileStatusReport, SwitchPlan};
use crate::agents::{Agent, ProfileMechanism};
use crate::cli::{AgentsAction, ProfileAction};
use crate::error_codes::CONFIG_ERROR;
use crate::output::{Outputable, print_and_exit};

pub fn run_agents_profile(action: &AgentsAction) -> i32 {
    match action {
        AgentsAction::Profile { action } => match action {
            ProfileAction::Init {} => init_action(),
            ProfileAction::Create { name, from, agents } => {
                create_action(name, from.as_deref(), agents.as_deref())
            }
            ProfileAction::Use {
                name,
                agents,
                global,
                print_env,
            } => use_action(name, agents.as_deref(), *global, *print_env),
            ProfileAction::List { output_format } => list_action(output_format),
            ProfileAction::Status { output_format } => status_action(output_format),
            ProfileAction::Delete { name } => delete_action(name),
        },
    }
}

// ---------------------------------------------------------------------------
// init

fn init_action() -> i32 {
    // Ensure the 'default' profile dir exists even when no agents are
    // installed — an empty store must still be listable. Idempotent.
    if let Err(e) = agent_profiles::create_profile("default", None)
        && !matches!(e, ProfileError::ProfileExists(_))
    {
        return fail(&e);
    }
    let snapshotted = match agent_profiles::init_snapshot("default") {
        Ok(agents) => agents,
        Err(e) => return fail(&e),
    };

    let mut registry = match ProfileRegistry::load() {
        Ok(r) => r,
        Err(e) => return fail(&e),
    };
    registry.default_profile = Some("default".to_string());
    for agent in &snapshotted {
        registry
            .active
            .insert(agent.slug().to_string(), "default".to_string());
    }
    if let Err(e) = registry.save() {
        return fail(&e);
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
    println!("Default profile set to 'default'.");
    emit_created(snapshotted.len());
    0
}

// ---------------------------------------------------------------------------
// create

fn create_action(name: &str, from: Option<&str>, agents_raw: Option<&str>) -> i32 {
    let only = match parse_optional_agents(agents_raw) {
        Ok(v) => v,
        Err(code) => return code,
    };

    // `if let Err` (Ok payload ignored) keeps this compatible with both
    // the `()` and `PathBuf` return shapes of the store primitive.
    if let Err(e) = agent_profiles::create_profile(name, from) {
        return fail(&e);
    }
    let pdir = match agent_profiles::profile_dir(name) {
        Ok(p) => p,
        Err(e) => return fail(&e),
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
                return CONFIG_ERROR;
            }
        } else {
            agent_count += 1;
        }
    }

    match from {
        Some(src) => {
            println!("Created profile '{name}' from '{src}' ({agent_count} agent snapshot(s)).")
        }
        None => println!("Created empty profile '{name}'."),
    }
    emit_created(agent_count);
    0
}

// ---------------------------------------------------------------------------
// use

fn use_action(name: &str, agents_raw: Option<&str>, global: bool, print_env: bool) -> i32 {
    let started = Instant::now();
    let explicit = agents_raw.is_some();
    let requested: Vec<Agent> = match parse_optional_agents(agents_raw) {
        Ok(Some(list)) => list,
        Ok(None) => Agent::ALL.to_vec(),
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

    let mut targets: Vec<Agent> = Vec::new();
    for agent in requested {
        if !agent.profile_switchable_v1() {
            if explicit {
                eprintln!(
                    "note: {} is not yet switchable (PRD-058 v1.1)",
                    agent.slug()
                );
            }
            continue;
        }
        targets.push(agent);
    }
    if targets.is_empty() {
        eprintln!(
            "Error: no v1-switchable agents selected (switchable: claude-code, codex, cursor)"
        );
        return CONFIG_ERROR;
    }

    let symlink_requested: Vec<Agent> = targets
        .iter()
        .copied()
        .filter(|a| is_symlink_tier(*a))
        .collect();

    let plans = match agent_profiles::plan_use(name, &targets, global) {
        Ok(p) => p,
        Err(e) => return fail(&e),
    };

    let mut exports: Vec<(&'static str, PathBuf)> = Vec::new();
    let mut repointed: Vec<Agent> = Vec::new();
    for plan in plans {
        match plan {
            SwitchPlan::EnvExports(vars) => exports.extend(vars),
            SwitchPlan::SymlinkRepoint { agent, target } => {
                if global {
                    if let Err(e) = agent_profiles::apply_symlink_repoint(agent, &target) {
                        return fail(&e);
                    }
                    repointed.push(agent);
                }
                // !global: covered by the stderr notice below.
            }
        }
    }

    let export_block = format_exports(&exports);
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
            Err(e) => return fail(&e),
        };
        for agent in &targets {
            registry
                .active
                .insert(agent.slug().to_string(), name.to_string());
        }
        registry.default_profile = Some(name.to_string());
        if let Err(e) = registry.save() {
            return fail(&e);
        }
        say(&format!("Default profile set to '{name}'."));
    }

    emit_switched(
        &targets,
        &repointed,
        global,
        started.elapsed().as_millis() as u64,
    );
    0
}

/// Bash-syntax export lines for the env tier. Paths are double-quoted
/// with `"` `\` `$` `` ` `` escaped so `eval`-ing the output can't be
/// tricked by a hostile path segment.
fn format_exports(exports: &[(&'static str, PathBuf)]) -> String {
    let mut out = String::new();
    for (var, path) in exports {
        let raw = path.display().to_string();
        let mut escaped = String::with_capacity(raw.len());
        for c in raw.chars() {
            if matches!(c, '"' | '\\' | '$' | '`') {
                escaped.push('\\');
            }
            escaped.push(c);
        }
        let _ = writeln!(out, "export {var}=\"{escaped}\"");
    }
    out
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
        Err(e) => fail(&e),
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
            // AgentProfileStatus fields are WP-A-owned; render generically
            // off the serialized shape so this stays correct as they evolve.
            for agent in &self.0.agents {
                if let Ok(serde_json::Value::Object(map)) = serde_json::to_value(agent) {
                    let line = map
                        .iter()
                        .map(|(k, v)| format!("{k}={}", render_scalar(v)))
                        .collect::<Vec<_>>()
                        .join("  ");
                    let _ = writeln!(out, "  {line}");
                }
            }
        }
        out
    }
}

fn render_scalar(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => "-".to_string(),
        other => other.to_string(),
    }
}

fn status_action(output_format: &str) -> i32 {
    match agent_profiles::collect_status() {
        Ok(report) => print_and_exit(StatusOutput(report), output_format),
        Err(e) => fail(&e),
    }
}

// ---------------------------------------------------------------------------
// delete

fn delete_action(name: &str) -> i32 {
    // Count snapshots up front for the telemetry payload (the dir is
    // gone after a successful delete; the name itself is never logged).
    let agent_count = agent_profiles::profile_dir(name)
        .map(|p| {
            Agent::ALL
                .iter()
                .filter(|a| p.join(a.slug()).exists())
                .count()
        })
        .unwrap_or(0);

    match agent_profiles::delete_profile(name) {
        Ok(()) => {
            println!("Deleted profile '{name}'.");
            emit_deleted(agent_count);
            0
        }
        Err(e @ ProfileError::ActiveProfileDelete(_)) => {
            eprintln!("Error: {e}");
            eprintln!(
                "Switch the affected agent(s) first: `jarvy agents profile use <other> --global`."
            );
            CONFIG_ERROR
        }
        Err(e) => fail(&e),
    }
}

// ---------------------------------------------------------------------------
// shared helpers

/// Every `ProfileError` maps to CONFIG_ERROR (2) — parity with
/// `skills_cmd`, which uses the same code for its IO failures.
fn fail(e: &ProfileError) -> i32 {
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

fn is_symlink_tier(agent: Agent) -> bool {
    agent
        .profile_mechanisms()
        .iter()
        .any(|m| matches!(m, ProfileMechanism::Symlink))
}

fn tier_label(agent: Agent) -> &'static str {
    if is_symlink_tier(agent) {
        "symlink tier — switches globally"
    } else {
        "env tier — per-terminal"
    }
}

// ---------------------------------------------------------------------------
// telemetry (gated; profile names NEVER emitted)

fn emit_created(agent_count: usize) {
    if crate::observability::telemetry_gate::is_enabled() {
        tracing::info!(event = "agent_profile.created", agent_count);
    }
}

fn emit_deleted(agent_count: usize) {
    if crate::observability::telemetry_gate::is_enabled() {
        tracing::info!(event = "agent_profile.deleted", agent_count);
    }
}

/// One `agent_profile.switched` event per mechanism so the `mechanism`
/// label stays bounded ("env" | "symlink") instead of a joined string.
fn emit_switched(targets: &[Agent], repointed: &[Agent], global: bool, duration_ms: u64) {
    if !crate::observability::telemetry_gate::is_enabled() {
        return;
    }
    let scope = if global { "global" } else { "tty" };
    let env_agents: Vec<&str> = targets
        .iter()
        .filter(|a| !is_symlink_tier(**a))
        .map(|a| a.slug())
        .collect();
    if !env_agents.is_empty() {
        tracing::info!(
            event = "agent_profile.switched",
            agents = env_agents.join(","),
            mechanism = "env",
            scope,
            duration_ms,
        );
    }
    if !repointed.is_empty() {
        tracing::info!(
            event = "agent_profile.switched",
            agents = slug_list(repointed),
            mechanism = "symlink",
            scope = "global",
            duration_ms,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        ]);
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
        assert!(format_exports(&[]).is_empty());
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
        let _home = pin_jarvy_home();
        let code = run_agents_profile(&profile_cmd(ProfileAction::Init {}));
        assert_eq!(code, 0);

        assert!(agent_profiles::profile_dir("default").unwrap().is_dir());
        let registry = ProfileRegistry::load().unwrap();
        assert_eq!(registry.default_profile.as_deref(), Some("default"));

        // Idempotent re-run.
        assert_eq!(run_agents_profile(&profile_cmd(ProfileAction::Init {})), 0);
        unpin_jarvy_home();
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn create_shows_up_in_list_json_and_refuses_dup() {
        let _home = pin_jarvy_home();
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
        unpin_jarvy_home();
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn create_with_unknown_agent_slug_is_config_error() {
        let _home = pin_jarvy_home();
        let code = run_agents_profile(&profile_cmd(ProfileAction::Create {
            name: "narrow".into(),
            from: None,
            agents: Some("claude-code,bogus".into()),
        }));
        assert_eq!(code, CONFIG_ERROR);
        // Nothing created on refusal.
        assert!(!agent_profiles::profile_dir("narrow").unwrap().exists());
        unpin_jarvy_home();
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn delete_removes_profile_and_second_delete_errors() {
        let _home = pin_jarvy_home();
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
        unpin_jarvy_home();
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn use_unknown_profile_is_config_error() {
        let _home = pin_jarvy_home();
        let code = run_agents_profile(&profile_cmd(ProfileAction::Use {
            name: "missing".into(),
            agents: Some("claude-code".into()),
            global: false,
            print_env: true,
        }));
        assert_eq!(code, CONFIG_ERROR);
        unpin_jarvy_home();
    }

    /// Seed `<profile>/<slug>/` directly in the store.
    fn seed_snapshot(profile: &str, agent: Agent) -> PathBuf {
        let dir = agent_profiles::profile_dir(profile)
            .unwrap()
            .join(agent.slug());
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn use_env_tier_exports_carry_profile_path_and_handler_exits_zero() {
        let _home = pin_jarvy_home();
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
        let block = format_exports(&exports);
        assert!(block.starts_with("export CLAUDE_CONFIG_DIR=\""));
        assert!(block.contains(&snap.display().to_string()));

        // Handler-level: same selection succeeds end-to-end.
        let code = run_agents_profile(&profile_cmd(ProfileAction::Use {
            name: "work".into(),
            agents: Some("claude-code".into()),
            global: false,
            print_env: true,
        }));
        assert_eq!(code, 0);
        unpin_jarvy_home();
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn use_cursor_only_without_global_exits_zero() {
        let _home = pin_jarvy_home();
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
        }));
        assert_eq!(code, 0);
        // Nothing was repointed (no --global): no live symlink.
        assert!(std::fs::symlink_metadata(Agent::Cursor.config_dir().unwrap()).is_err());
        unpin_jarvy_home();
    }

    #[cfg(unix)]
    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn delete_active_symlink_profile_via_handler_is_config_error() {
        let _home = pin_jarvy_home();
        // Seed a live cursor dir, init so it's moved + linked back.
        std::fs::create_dir_all(Agent::Cursor.config_dir().unwrap()).unwrap();
        assert_eq!(run_agents_profile(&profile_cmd(ProfileAction::Init {})), 0);

        let code = run_agents_profile(&profile_cmd(ProfileAction::Delete {
            name: "default".into(),
        }));
        assert_eq!(code, CONFIG_ERROR);
        // Profile survives the refusal.
        assert!(agent_profiles::profile_dir("default").unwrap().is_dir());
        unpin_jarvy_home();
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn create_from_missing_source_is_config_error() {
        let _home = pin_jarvy_home();
        let code = run_agents_profile(&profile_cmd(ProfileAction::Create {
            name: "new".into(),
            from: Some("missing".into()),
            agents: None,
        }));
        assert_eq!(code, CONFIG_ERROR);
        // Nothing left behind for the refused create.
        assert!(!agent_profiles::profile_dir("new").unwrap().exists());
        unpin_jarvy_home();
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn create_agents_narrowing_prunes_snapshots() {
        let _home = pin_jarvy_home();
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
        unpin_jarvy_home();
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn status_json_shape_is_stable() {
        let _home = pin_jarvy_home();
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
            "switchable_v1",
            "active_profile",
            "state",
        ] {
            assert!(entry.contains_key(key), "missing agent key {key}");
        }
        unpin_jarvy_home();
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn init_twice_env_tier_refreshes_snapshot_from_live() {
        // Documents actual handler-level idempotency: a second init
        // re-COPIES the live env-tier dir over the snapshot (save
        // semantics). Unchanged live == unchanged snapshot; a changed
        // live dir is re-captured. Exit 0 both times.
        let _home = pin_jarvy_home();
        let live = Agent::ClaudeCode.config_dir().unwrap();
        std::fs::create_dir_all(&live).unwrap();
        std::fs::write(live.join("settings.json"), "v1").unwrap();
        assert_eq!(run_agents_profile(&profile_cmd(ProfileAction::Init {})), 0);

        let snap = agent_profiles::profile_dir("default")
            .unwrap()
            .join("claude-code")
            .join("settings.json");
        assert_eq!(std::fs::read_to_string(&snap).unwrap(), "v1");

        // Unchanged live: second init leaves the snapshot content intact.
        assert_eq!(run_agents_profile(&profile_cmd(ProfileAction::Init {})), 0);
        assert_eq!(std::fs::read_to_string(&snap).unwrap(), "v1");
        // Live dir itself untouched (still a real dir with content).
        assert_eq!(
            std::fs::read_to_string(live.join("settings.json")).unwrap(),
            "v1"
        );

        // Changed live: a third init re-captures the new content.
        std::fs::write(live.join("settings.json"), "v2").unwrap();
        assert_eq!(run_agents_profile(&profile_cmd(ProfileAction::Init {})), 0);
        assert_eq!(std::fs::read_to_string(&snap).unwrap(), "v2");
        unpin_jarvy_home();
    }
}
