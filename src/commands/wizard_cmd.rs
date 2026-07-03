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

    // Stat `jarvy.toml` before + after the spawn so
    // `wizard.headless_exit` can distinguish silent-success paths:
    //   - agent honored the "already configured" step-2 no-op → file unchanged
    //   - agent completed the playbook → file modified
    //   - agent crashed mid-call → file unchanged, exit non-zero
    //   - agent misread the plan → file unchanged, exit zero (bug signal)
    // Without these fields, on-call debugging "why did the wizard
    // exit 0 without writing jarvy.toml?" has literally no signal to
    // correlate against (Obs F3).
    let jarvy_toml_path = std::path::PathBuf::from(&opts.config_file);
    let before_state = stat_jarvy_toml(&jarvy_toml_path);

    // `mcp_preapproval` records what allowlist scope was passed to
    // the child agent's CLI — mirrors the actual argv, not the docs.
    // CLAUDE.md event taxonomy for `wizard.headless_spawned` was
    // updated as part of this fix: `cmd_argv0` alone is no longer
    // sufficient once we silently add `--allowedTools`.
    let mcp_preapproval = if agent == crate::agents::Agent::ClaudeCode {
        "mcp__jarvy"
    } else {
        ""
    };

    // Per-invocation session UUID. Threaded through:
    //   1. `session::WizardSessionGuard::activate(session_id)` writes
    //      the marker file at ~/.jarvy/state/wizard-session-<id>.active
    //      and removes it on Drop (including panic unwinds).
    //   2. `headless::run(agent, prompt, session_id)` exports the UUID
    //      via `JARVY_WIZARD_SESSION_ID` on the agent spawn.
    //   3. Descendant `jarvy mcp` server processes read the env var
    //      + verify the marker still exists (and is fresh) via
    //      `session::is_active()` in `gate_mutation`.
    //
    // The guard is scoped to this function — dropping it removes the
    // marker even if the agent leaves long-lived children carrying the
    // env var. Those children fail `session::is_active()` and fall
    // through to the normal confirmation gate (or refuse if they
    // can't prompt).
    //
    // Generated BEFORE the spawned event so on-call can correlate the
    // whole lifecycle (spawned → mcp.mutation.wizard_bypass →
    // discover.applied → headless_exit) via a single UUID field.
    let session_id = super::super::wizard::headless::new_session_id();
    let _session_guard = crate::wizard::session::WizardSessionGuard::activate(&session_id);

    if crate::observability::telemetry_gate::is_enabled() {
        tracing::info!(
            event = "wizard.headless_spawned",
            agent = agent.slug(),
            cmd_argv0 = cli_command,
            mcp_preapproval = mcp_preapproval,
            wizard_session_env = true,
            wizard_session_id = %session_id,
        );
    }

    let start = std::time::Instant::now();
    let exit_status: std::process::ExitStatus =
        match headless::run(agent, &prompt_body, &session_id) {
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

    let after_state = stat_jarvy_toml(&jarvy_toml_path);
    let (jarvy_toml_before, jarvy_toml_after, terminal_state) =
        classify_headless_outcome(&before_state, &after_state, exit_status.code());

    if crate::observability::telemetry_gate::is_enabled() {
        tracing::info!(
            event = "wizard.headless_exit",
            agent = agent.slug(),
            exit_code = exit_status.code().unwrap_or(-1),
            wall_ms,
            jarvy_toml_before = jarvy_toml_before,
            jarvy_toml_after = jarvy_toml_after,
            terminal_state = terminal_state,
            wizard_session_id = %session_id,
        );
    }
    exit_status.code().unwrap_or(error_codes::CONFIG_ERROR)
}

/// Snapshot of `jarvy.toml` state used to classify wizard outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
enum JarvyTomlSnapshot {
    /// File didn't exist at the time of stat.
    Absent,
    /// File exists; carry byte length + last-modified secs as a cheap
    /// fingerprint. `sha256` would be more precise but is overkill for
    /// distinguishing "file wasn't touched" from "file was rewritten".
    Present { bytes: u64, mtime_secs: i64 },
}

fn stat_jarvy_toml(path: &std::path::Path) -> JarvyTomlSnapshot {
    match std::fs::metadata(path) {
        Ok(meta) => {
            let mtime_secs = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            JarvyTomlSnapshot::Present {
                bytes: meta.len(),
                mtime_secs,
            }
        }
        Err(_) => JarvyTomlSnapshot::Absent,
    }
}

// Pinned string values for the `terminal_state` field on the
// `wizard.headless_exit` telemetry event. On-call runbooks alert on
// these strings — pinning them here means a refactor renaming
// `"early_exit"` to something else breaks compile-time (via the
// exhaustive test below) instead of silently rerouting every
// downstream query. Referenced by CLAUDE.md event taxonomy for
// `wizard.headless_exit`.
pub const TERMINAL_STATE_PLAYBOOK_COMPLETED: &str = "playbook_completed";
pub const TERMINAL_STATE_NOOP_ALREADY_CONFIGURED: &str = "noop_already_configured";
pub const TERMINAL_STATE_EARLY_EXIT: &str = "early_exit";
pub const TERMINAL_STATE_UNKNOWN: &str = "unknown";

fn classify_headless_outcome(
    before: &JarvyTomlSnapshot,
    after: &JarvyTomlSnapshot,
    exit_code: Option<i32>,
) -> (&'static str, &'static str, &'static str) {
    let before_str = match before {
        JarvyTomlSnapshot::Absent => "absent",
        JarvyTomlSnapshot::Present { .. } => "present",
    };
    let after_str = match (before, after) {
        (_, JarvyTomlSnapshot::Absent) => "absent",
        (JarvyTomlSnapshot::Absent, JarvyTomlSnapshot::Present { .. }) => "created",
        (a, b) if a == b => "unchanged",
        _ => "modified",
    };
    let terminal_state = match (before, after, exit_code) {
        // Playbook wrote/modified jarvy.toml AND exited cleanly →
        // typical happy path.
        (_, JarvyTomlSnapshot::Present { .. }, Some(0)) if !matches!((before, after), (a, b) if a == b) => {
            TERMINAL_STATE_PLAYBOOK_COMPLETED
        }
        // File unchanged + clean exit → step-2 no-op branch of the
        // playbook (project already configured).
        (a, b, Some(0)) if a == b => TERMINAL_STATE_NOOP_ALREADY_CONFIGURED,
        // File unchanged + non-zero exit → agent errored before
        // touching state (recoverable, but on-call worth-a-look).
        (a, b, code) if a == b && code != Some(0) => TERMINAL_STATE_EARLY_EXIT,
        _ => TERMINAL_STATE_UNKNOWN,
    };
    (before_str, after_str, terminal_state)
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod classify_tests {
    use super::*;

    /// Table-driven exhaustive coverage of the 4-way outcome match.
    /// A refactor that inverts an arm, "cleans up" the `matches!`
    /// guard, or renames a terminal_state constant trips at least
    /// one row.
    #[test]
    fn classify_headless_outcome_covers_all_terminal_states() {
        let absent = JarvyTomlSnapshot::Absent;
        let old = JarvyTomlSnapshot::Present {
            bytes: 100,
            mtime_secs: 1_000,
        };
        let new = JarvyTomlSnapshot::Present {
            bytes: 200,
            mtime_secs: 2_000,
        };
        // (before, after, exit) → (before_str, after_str, terminal_state)
        #[allow(clippy::type_complexity)]
        let cases: &[(
            &JarvyTomlSnapshot,
            &JarvyTomlSnapshot,
            Option<i32>,
            &str,
            &str,
            &str,
        )] = &[
            // Playbook completed: absent → created OR present → modified,
            // both with exit 0.
            (
                &absent,
                &new,
                Some(0),
                "absent",
                "created",
                TERMINAL_STATE_PLAYBOOK_COMPLETED,
            ),
            (
                &old,
                &new,
                Some(0),
                "present",
                "modified",
                TERMINAL_STATE_PLAYBOOK_COMPLETED,
            ),
            // Noop already configured: file unchanged AND clean exit.
            (
                &old,
                &old,
                Some(0),
                "present",
                "unchanged",
                TERMINAL_STATE_NOOP_ALREADY_CONFIGURED,
            ),
            (
                &absent,
                &absent,
                Some(0),
                "absent",
                "absent",
                TERMINAL_STATE_NOOP_ALREADY_CONFIGURED,
            ),
            // Early exit: file unchanged AND non-zero exit.
            (
                &old,
                &old,
                Some(1),
                "present",
                "unchanged",
                TERMINAL_STATE_EARLY_EXIT,
            ),
            (
                &absent,
                &absent,
                Some(1),
                "absent",
                "absent",
                TERMINAL_STATE_EARLY_EXIT,
            ),
            // Unknown: no exit code (signal-killed) with file changed.
            (
                &old,
                &new,
                None,
                "present",
                "modified",
                TERMINAL_STATE_UNKNOWN,
            ),
        ];
        for (before, after, exit, want_before, want_after, want_state) in cases {
            let got = classify_headless_outcome(before, after, *exit);
            assert_eq!(
                got,
                (*want_before, *want_after, *want_state),
                "case (before={before:?}, after={after:?}, exit={exit:?}): \
                 expected ({want_before}, {want_after}, {want_state}), got {got:?}"
            );
        }
    }
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
