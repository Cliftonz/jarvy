//! `jarvy skills <action>` command handler (PRD-049 v1 + phase 2
//! update / remove / ad-hoc install).

use crate::cli::SkillsAction;
use crate::config::Config;
use crate::skills::{self, SkillAgent, SkillEntry, SkillStatus, SkillsConfig};

pub fn run_skills(action: &SkillsAction, file: &str) -> i32 {
    let config = Config::new(file);
    let skills_cfg = config.skills.clone().unwrap_or_default();

    match action {
        SkillsAction::Install {
            name,
            output_format,
        } => install_action(&skills_cfg, name.as_deref(), output_format),
        SkillsAction::Update {
            name,
            output_format,
        } => update_action(&skills_cfg, name.as_deref(), output_format),
        SkillsAction::Remove {
            name,
            output_format,
        } => remove_action(&skills_cfg, name, output_format),
        SkillsAction::List {} => list_action(&skills_cfg),
        SkillsAction::Status {} => status_action(&skills_cfg),
        SkillsAction::Agents {} => agents_action(),
    }
}

fn agent_slugs(agents: &[SkillAgent]) -> Vec<&'static str> {
    agents.iter().map(|a| a.slug()).collect()
}

/// Resolve the entries an install/update run should operate on.
///
/// - No name → every `[skills.install]` entry.
/// - Named + configured → that entry (pinned version honored).
/// - Named + NOT configured → an ad-hoc `latest` entry resolved from
///   library_sources (PRD-049 phase 2). `ad_hoc = true` in the result.
fn select_entries<'a>(
    cfg: &'a SkillsConfig,
    only_name: Option<&'a str>,
    adhoc_slot: &'a mut Option<SkillEntry>,
) -> Vec<(&'a str, &'a SkillEntry, bool)> {
    match only_name {
        Some(want) => match cfg.install.get(want) {
            Some(entry) => vec![(want, entry, false)],
            None => {
                let entry = adhoc_slot.insert(SkillEntry::Version("latest".to_string()));
                vec![(want, entry, true)]
            }
        },
        None => cfg
            .install
            .iter()
            .map(|(name, entry)| (name.as_str(), entry, false))
            .collect(),
    }
}

fn install_action(cfg: &SkillsConfig, only_name: Option<&str>, output_format: &str) -> i32 {
    let json = output_format == "json";
    if cfg.install.is_empty() && only_name.is_none() {
        if json {
            print_json(&serde_json::json!({
                "status": "noop",
                "reason": "no_skills_configured",
            }));
        } else {
            println!(
                "No skills configured. Add entries to `[skills.install]` in jarvy.toml \
                 or pass a skill name (`jarvy skills install <name>`)."
            );
        }
        return 0;
    }

    prepare_library_sources(cfg);

    let agents = resolve_target_agents(cfg);
    if agents.is_empty() {
        return no_agents_error(json);
    }

    let mut adhoc_slot = None;
    let entries = select_entries(cfg, only_name, &mut adhoc_slot);

    let mut installed = Vec::new();
    let mut failures = Vec::new();
    for (name, entry, ad_hoc) in entries {
        match skills::install_skill(name, entry, &agents) {
            Ok(result) => {
                if !json {
                    println!(
                        "  Installed {} {} → {} agent(s){}",
                        name,
                        result.version,
                        result.agents.len(),
                        if ad_hoc {
                            " (ad-hoc, not in jarvy.toml)"
                        } else {
                            ""
                        },
                    );
                    for skipped in &result.skipped_agents {
                        println!("    skipped {}: {}", skipped.0.slug(), skipped.1);
                    }
                }
                installed.push(serde_json::json!({
                    "name": name,
                    "version": result.version,
                    "ad_hoc": ad_hoc,
                    "agents": agent_slugs(&result.agents),
                    "skipped": result.skipped_agents.iter().map(|(a, reason)| {
                        serde_json::json!({ "agent": a.slug(), "reason": reason })
                    }).collect::<Vec<_>>(),
                }));
            }
            Err(e) => {
                if !json {
                    eprintln!("  Failed: {name}: {e}");
                }
                failures.push(serde_json::json!({
                    "name": name,
                    "error_kind": e.kind(),
                    "error": e.to_string(),
                }));
            }
        }
    }

    let had_failure = !failures.is_empty();
    if json {
        print_json(&serde_json::json!({
            "status": if had_failure { "failed" } else { "ok" },
            "installed": installed,
            "failures": failures,
        }));
    }
    if had_failure {
        crate::error_codes::CONFIG_ERROR
    } else {
        0
    }
}

fn update_action(cfg: &SkillsConfig, only_name: Option<&str>, output_format: &str) -> i32 {
    let json = output_format == "json";
    if cfg.install.is_empty() && only_name.is_none() {
        if json {
            print_json(&serde_json::json!({
                "status": "noop",
                "reason": "no_skills_configured",
            }));
        } else {
            println!(
                "No skills configured. Add entries to `[skills.install]` in jarvy.toml \
                 or pass a skill name (`jarvy skills update <name>`)."
            );
        }
        return 0;
    }

    prepare_library_sources(cfg);

    let agents = resolve_target_agents(cfg);
    if agents.is_empty() {
        return no_agents_error(json);
    }

    let mut adhoc_slot = None;
    let entries = select_entries(cfg, only_name, &mut adhoc_slot);

    let mut updated = Vec::new();
    let mut failures = Vec::new();
    for (name, entry, _ad_hoc) in entries {
        match skills::update_skill(name, entry, &agents) {
            Ok(result) => {
                if !json {
                    if result.updated_agents.is_empty() {
                        println!(
                            "  {} {} — up to date ({} agent(s))",
                            name,
                            result.version,
                            result.unchanged_agents.len()
                        );
                    } else {
                        println!(
                            "  Updated {} → {} ({} agent(s), {} already current)",
                            name,
                            result.version,
                            result.updated_agents.len(),
                            result.unchanged_agents.len()
                        );
                    }
                    for skipped in &result.skipped_agents {
                        println!("    skipped {}: {}", skipped.0.slug(), skipped.1);
                    }
                }
                updated.push(serde_json::json!({
                    "name": name,
                    "version": result.version,
                    "updated_agents": agent_slugs(&result.updated_agents),
                    "unchanged_agents": agent_slugs(&result.unchanged_agents),
                    "skipped": result.skipped_agents.iter().map(|(a, reason)| {
                        serde_json::json!({ "agent": a.slug(), "reason": reason })
                    }).collect::<Vec<_>>(),
                }));
            }
            Err(e) => {
                if !json {
                    eprintln!("  Failed: {name}: {e}");
                }
                failures.push(serde_json::json!({
                    "name": name,
                    "error_kind": e.kind(),
                    "error": e.to_string(),
                }));
            }
        }
    }

    let had_failure = !failures.is_empty();
    if json {
        print_json(&serde_json::json!({
            "status": if had_failure { "failed" } else { "ok" },
            "skills": updated,
            "failures": failures,
        }));
    }
    if had_failure {
        crate::error_codes::CONFIG_ERROR
    } else {
        0
    }
}

fn remove_action(cfg: &SkillsConfig, name: &str, output_format: &str) -> i32 {
    let json = output_format == "json";

    // No library sync needed — removal is purely local.
    let agents = resolve_target_agents(cfg);
    if agents.is_empty() {
        return no_agents_error(json);
    }

    match skills::remove_skill(name, &agents) {
        Ok(result) => {
            if json {
                print_json(&serde_json::json!({
                    "status": "ok",
                    "skill": name,
                    "removed_agents": agent_slugs(&result.removed_agents),
                    "absent_agents": agent_slugs(&result.absent_agents),
                }));
            } else if result.removed_agents.is_empty() {
                println!("  {name} was not installed for any targeted agent — nothing to do.");
            } else {
                println!(
                    "  Removed {} from {} agent(s)",
                    name,
                    result.removed_agents.len()
                );
                for agent in &result.removed_agents {
                    println!("    {}", agent.slug());
                }
                if !result.absent_agents.is_empty() {
                    println!(
                        "  (not installed for: {})",
                        agent_slugs(&result.absent_agents).join(", ")
                    );
                }
            }
            0
        }
        Err(e) => {
            if json {
                print_json(&serde_json::json!({
                    "status": "failed",
                    "skill": name,
                    "error_kind": e.kind(),
                    "error": e.to_string(),
                }));
            } else {
                eprintln!("  Failed: {name}: {e}");
            }
            crate::error_codes::CONFIG_ERROR
        }
    }
}

fn no_agents_error(json: bool) -> i32 {
    if json {
        print_json(&serde_json::json!({
            "status": "failed",
            "error_kind": "no_agents",
            "error": "no AI agents detected",
        }));
    } else {
        eprintln!(
            "No AI agents detected. Install Claude Code / Cursor / Codex / etc. first, \
             or check `jarvy skills agents`."
        );
    }
    crate::error_codes::CONFIG_ERROR
}

fn print_json(value: &serde_json::Value) {
    println!(
        "{}",
        serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".into())
    );
}

fn list_action(cfg: &SkillsConfig) -> i32 {
    if cfg.install.is_empty() {
        println!("No skills configured in `[skills.install]`.");
        return 0;
    }
    println!("Configured skills ({}):", cfg.install.len());
    let agents = resolve_target_agents(cfg);
    for (name, entry) in &cfg.install {
        println!();
        println!("  {} = {}", name, entry.version());
        for agent in &agents {
            let status = skills::installer::skill_status(name, entry.version(), *agent);
            let label = match status {
                SkillStatus::Installed { version } => format!("installed ({version})"),
                SkillStatus::Missing => "missing".to_string(),
                SkillStatus::Drift {
                    installed,
                    requested,
                } => format!("drift: installed={installed} requested={requested}"),
            };
            println!("    {} → {}", agent.slug(), label);
        }
    }
    0
}

fn status_action(cfg: &SkillsConfig) -> i32 {
    let agents = resolve_target_agents(cfg);
    let mut drift_count = 0;
    let mut missing_count = 0;
    let mut installed_count = 0;
    for (name, entry) in &cfg.install {
        for agent in &agents {
            match skills::installer::skill_status(name, entry.version(), *agent) {
                SkillStatus::Installed { .. } => installed_count += 1,
                SkillStatus::Missing => missing_count += 1,
                SkillStatus::Drift { .. } => drift_count += 1,
            }
        }
    }
    println!("Skills Status");
    println!("=============");
    println!("Installed: {installed_count}");
    println!("Missing:   {missing_count}");
    println!("Drift:     {drift_count}");
    if drift_count > 0 || missing_count > 0 {
        println!();
        println!("Run `jarvy skills install` to install missing skills.");
    }
    0
}

fn agents_action() -> i32 {
    let agents = skills::detect_agents();
    println!("Detected AI agents:");
    if agents.is_empty() {
        println!("  (none)");
        return 0;
    }
    for a in agents {
        println!("  {} ({})", a.slug(), a.config_dir().unwrap().display());
    }
    0
}

fn resolve_target_agents(cfg: &SkillsConfig) -> Vec<SkillAgent> {
    if cfg.agents.is_empty() {
        return skills::detect_agents();
    }
    cfg.agents
        .iter()
        .filter_map(|slug| SkillAgent::from_slug(slug))
        .collect()
}

fn prepare_library_sources(cfg: &SkillsConfig) {
    crate::library_registry::sync_all("skills", "", &cfg.library_sources, cfg.origin);
}
