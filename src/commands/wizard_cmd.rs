//! `jarvy wizard` command handler.
//!
//! Thin glue between the CLI args and the `wizard` module. All
//! business logic — mode picking, prompt assembly, agent spawn —
//! lives under `src/wizard/`. This module owns:
//!
//! - argument shaping (`Option<String>` → typed slug)
//! - human-readable output (the JSON path is handled symmetrically)
//! - telemetry emission (gated on `telemetry_gate::is_enabled()`)
//! - exit-code mapping
//!
//! Quickstart fallback: when no AI agent is installed AND no
//! `--agent` override is supplied, we delegate to
//! `commands::quickstart::run_quickstart` rather than refusing — the
//! existing first-run flow is the right UX for users without an
//! agent.

use crate::error_codes;
use crate::wizard::{self, WizardMode, WizardOpts, context, headless, prompt, skill_drop};

/// Args struct mirrors the clap `Commands::Wizard` variant.
pub struct WizardCliArgs<'a> {
    pub agent: Option<&'a str>,
    pub skill_only: bool,
    pub apply: bool,
    pub output_format: &'a str,
    pub file: &'a str,
}

pub fn run(args: WizardCliArgs<'_>) -> i32 {
    let opts = WizardOpts {
        agent_override: args.agent.map(str::to_string),
        skill_only: args.skill_only,
        apply: args.apply,
        output_format: args.output_format.to_string(),
        config_file: args.file.to_string(),
    };

    // Trust boundary gates — refuse before any agent invocation.
    //
    // Override env var: `JARVY_WIZARD=1` opts the user in to wizard
    // runs in environments where it's normally refused (sandbox,
    // CI). Mirrors `JARVY_TELEMETRY`'s shape. Useful for CI
    // workflows that explicitly want to bootstrap config via an
    // agent (rare today but a documented escape hatch).
    let forced = std::env::var("JARVY_WIZARD").as_deref() == Ok("1");
    if !forced {
        if crate::sandbox::is_sandbox() {
            return refuse(&opts, "sandbox");
        }
        if crate::ci::is_ci() {
            return refuse(&opts, "ci");
        }
        use std::io::IsTerminal;
        if !std::io::stdin().is_terminal() && !opts.skill_only {
            // Skill-drop is TTY-agnostic; headless needs a TTY so
            // the user can answer agent prompts. Force skill-only
            // when there's no TTY.
            return refuse(&opts, "non_tty");
        }
    }

    // Remote-config refusal. If the supplied `--file` is a config
    // pulled from a remote source, the wizard previews only — never
    // auto-applies. Matches `[packages] allow_remote` posture.
    if let Some(text) = std::fs::read_to_string(&opts.config_file).ok()
        && let Ok(cfg) = toml::from_str::<toml::Table>(&text)
        && let Some(origin) = cfg.get("__origin").and_then(|v| v.as_str())
        && origin == "remote"
        && opts.apply
    {
        return refuse(&opts, "remote_config");
    }

    let installed = wizard::detect_installed_agents();
    let mode = wizard::pick_mode(&opts, &installed, |agent| {
        // `which::which` only returns Ok when the binary is on PATH;
        // any error (not-found, permission) collapses to false.
        match wizard::agent_cli_command(agent) {
            Some(cmd) => which::which(cmd).is_ok(),
            None => false,
        }
    });

    if crate::observability::telemetry_gate::is_enabled() {
        let mode_label = match &mode {
            WizardMode::Headless { .. } => "headless",
            WizardMode::SkillDrop { .. } => "skill_drop",
            WizardMode::QuickstartFallback => "quickstart_fallback",
        };
        let agent_label = match &mode {
            WizardMode::Headless { agent, .. } | WizardMode::SkillDrop { agent } => agent.slug(),
            WizardMode::QuickstartFallback => "none",
        };
        tracing::info!(
            event = "wizard.started",
            mode = mode_label,
            agent = agent_label,
            apply = opts.apply,
            skill_only = opts.skill_only,
        );
    }

    match mode {
        WizardMode::Headless { agent, cli_command } => run_headless(&opts, agent, &cli_command),
        WizardMode::SkillDrop { agent } => run_skill_drop(&opts, agent),
        WizardMode::QuickstartFallback => run_quickstart_fallback(&opts),
    }
}

fn run_skill_drop(opts: &WizardOpts, agent: crate::agents::Agent) -> i32 {
    match skill_drop::install(agent) {
        Ok(path) => {
            let path_str = path.display().to_string();
            if crate::observability::telemetry_gate::is_enabled() {
                tracing::info!(
                    event = "wizard.skill_dropped",
                    agent = agent.slug(),
                    skill_path = %path_str,
                );
            }
            let phrase = skill_drop::invocation_phrase(agent);
            if opts.output_format == "json" {
                println!(
                    "{}",
                    serde_json::json!({
                        "status": "ok",
                        "mode": "skill_drop",
                        "agent": agent.slug(),
                        "skill_path": path_str,
                        "next_action": format!("open {} and type: {}", agent.slug(), phrase),
                    })
                );
            } else {
                println!();
                println!("✓ Installed jarvy-setup skill for {}", agent.slug());
                println!("  Path: {}", path_str);
                println!();
                println!("Next: open {} and type:", agent.slug());
                println!("  \"{phrase}\"");
                println!();
                println!("Your agent will read jarvy.toml (or bootstrap one),");
                println!("propose changes, and apply them via Jarvy's MCP server.");
            }
            0
        }
        Err(e) => {
            eprintln!("wizard: skill drop failed: {e}");
            if crate::observability::telemetry_gate::is_enabled() {
                tracing::warn!(
                    event = "wizard.refused",
                    reason = "skill_drop_failed",
                    error = %e,
                );
            }
            error_codes::CONFIG_ERROR
        }
    }
}

fn run_headless(opts: &WizardOpts, agent: crate::agents::Agent, cli_command: &str) -> i32 {
    let project_dir = crate::paths::config_parent_dir(&opts.config_file);
    let discover_report = {
        // Recompute discover at wizard time — cheap, single-pass over
        // the project tree. Same call shape `jarvy discover` uses.
        let existing_text = std::fs::read_to_string(&opts.config_file).ok();
        let already_configured: std::collections::HashSet<String> = existing_text
            .as_deref()
            .and_then(|t| t.parse::<toml::Table>().ok())
            .and_then(|t| t.get("provisioner").and_then(|v| v.as_table()).cloned())
            .map(|t| t.keys().cloned().collect())
            .unwrap_or_default();
        let known: std::collections::HashSet<String> =
            crate::tools::registry::registered_tool_names()
                .into_iter()
                .collect();
        crate::discover::analyze(&project_dir, &already_configured, &known)
    };
    let ctx = context::build(&project_dir, discover_report);
    let prompt_body = prompt::build(&ctx, agent);

    if !opts.apply {
        // Preview mode: don't actually spawn the agent. Print the
        // prompt that *would* be sent, plus the proposed plan, so the
        // user can review.
        if opts.output_format == "json" {
            println!(
                "{}",
                serde_json::json!({
                    "status": "preview",
                    "mode": "headless",
                    "agent": agent.slug(),
                    "cli_command": cli_command,
                    "context": &ctx,
                })
            );
        } else {
            println!(
                "[preview only — pass --apply to actually launch {}]",
                cli_command
            );
            println!();
            println!("Detected ecosystems:");
            for d in &ctx.discover.detections {
                println!("  - {} ({})", d.tool, d.source);
            }
            println!();
            println!("Would launch: {} ...", cli_command);
            println!("Prompt length: {} bytes", prompt_body.len());
        }
        return 0;
    }

    if crate::observability::telemetry_gate::is_enabled() {
        tracing::info!(
            event = "wizard.headless_spawned",
            agent = agent.slug(),
            cmd_argv0 = cli_command,
        );
    }
    let start = std::time::Instant::now();
    let exit_status: std::process::ExitStatus = match headless::run(agent, &prompt_body) {
        Ok(st) => st,
        Err(e) => {
            eprintln!("wizard: headless spawn failed: {e}");
            if crate::observability::telemetry_gate::is_enabled() {
                tracing::warn!(
                    event = "wizard.refused",
                    reason = "headless_spawn_failed",
                    error = %e,
                );
            }
            return error_codes::CONFIG_ERROR;
        }
    };
    let wall_ms = start.elapsed().as_millis() as u64;
    if crate::observability::telemetry_gate::is_enabled() {
        tracing::info!(
            event = "wizard.headless_exit",
            agent = agent.slug(),
            exit_code = exit_status.code().unwrap_or(-1),
            wall_ms,
        );
    }
    exit_status.code().unwrap_or(error_codes::CONFIG_ERROR)
}

/// Refuse with a structured advisory. Telemetry-gated. Returns
/// a non-zero exit so wrappers see the failure.
fn refuse(opts: &WizardOpts, reason: &'static str) -> i32 {
    if crate::observability::telemetry_gate::is_enabled() {
        tracing::warn!(event = "wizard.refused", reason = reason);
    }
    if opts.output_format == "json" {
        println!(
            "{}",
            serde_json::json!({
                "status": "refused",
                "reason": reason,
                "override": "set JARVY_WIZARD=1 to force (only when you understand the trade-off)",
            })
        );
    } else {
        eprintln!("wizard: refused — {reason}");
        match reason {
            "sandbox" => eprintln!(
                "       Detected an AI agent sandbox. Wizard runs outside, on the host shell."
            ),
            "ci" => eprintln!(
                "       CI environment detected. The wizard expects an interactive agent."
            ),
            "non_tty" => eprintln!(
                "       Headless mode needs a TTY so you can answer the agent. \
                 Re-run with --skill-only to drop a SKILL.md instead."
            ),
            "remote_config" => eprintln!(
                "       Refusing to auto-apply against a remote config. \
                 Run without --apply to preview, or remove the remote tag."
            ),
            _ => {}
        }
        eprintln!("       Override: JARVY_WIZARD=1 (only when you understand the trade-off).");
    }
    crate::error_codes::CONFIG_ERROR
}

fn run_quickstart_fallback(opts: &WizardOpts) -> i32 {
    if opts.output_format == "json" {
        println!(
            "{}",
            serde_json::json!({
                "status": "quickstart_fallback",
                "reason": "no_ai_agent_detected",
                "next_action": "jarvy quickstart",
            })
        );
    } else {
        eprintln!("wizard: no AI agent detected on this machine.");
        eprintln!("       Falling back to `jarvy quickstart` — the same first-run");
        eprintln!("       experience users without an agent get.\n");
    }
    if crate::observability::telemetry_gate::is_enabled() {
        tracing::info!(
            event = "wizard.refused",
            reason = "no_agent_installed",
            fallback = "quickstart",
        );
    }
    // Delegate to quickstart with default flags. Mirrors what the CLI
    // would do if the user typed `jarvy quickstart` directly.
    let result = crate::commands::quickstart::run_quickstart(
        crate::commands::quickstart::QuickstartOptions::default(),
    );
    crate::output::print_and_exit(result, &opts.output_format)
}
