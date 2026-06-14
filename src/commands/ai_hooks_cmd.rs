//! `jarvy ai-hooks` command handler.
//!
//! Subcommands:
//!
//! - `list`            — show what's in jarvy.toml + agent → path mapping
//! - `list --library`  — show built-in library hooks
//! - `apply`           — write hook configs to every targeted agent
//! - `check`           — diff desired vs. on-disk state (exit 1 if drift)
//! - `remove`          — strip jarvy-managed entries from every agent
//! - `test <name>`     — pipe a sample agent payload through a library hook

use std::fs;
use std::time::Instant;

use crate::ai_hooks::{
    AiHooksConfig, ConfigOrigin, HookScope, apply, check, library, remove, runner,
};
use crate::cli::AiHooksAction;
use crate::config::Config;
use crate::telemetry;

pub fn run_ai_hooks(action: &AiHooksAction, file: &str) -> i32 {
    match action {
        AiHooksAction::List { library } => run_list(*library, file),
        AiHooksAction::Apply { scope } => run_apply(scope.as_deref(), file),
        AiHooksAction::Check { scope } => run_check(scope.as_deref(), file),
        AiHooksAction::Remove { scope } => run_remove(scope.as_deref(), file),
        AiHooksAction::Test { name } => run_test(name),
    }
}

fn run_list(show_library: bool, file: &str) -> i32 {
    if show_library {
        println!("Built-in AI hook library:\n");
        for hook in library::LIBRARY {
            println!("  {:<32} {}", hook.name, hook.description);
            println!(
                "  {:<32}   event={} matcher={:?} timeout={}ms",
                "",
                hook.event,
                hook.matcher.unwrap_or("(any)"),
                hook.timeout_ms,
            );
            println!();
        }
        return 0;
    }

    let Some(cfg) = load_with_scope(file, None) else {
        eprintln!("No [ai_hooks] section in {file}");
        return 0;
    };
    println!("AI hooks configuration ({file}):");
    println!("  agents: {:?}", cfg.unique_agents());
    println!("  scope:  {:?}", cfg.scope);
    println!("  allow_custom_commands: {}", cfg.allow_custom_commands);
    println!("  origin: {:?}", cfg.origin);
    println!("  hooks:");
    for hook in &cfg.hooks {
        let kind = if hook.is_library() {
            "library"
        } else if hook.is_custom_command() {
            "custom"
        } else {
            "invalid"
        };
        println!("    - {} ({kind})", hook.identifier());
    }
    let refused = runner::audit_custom_commands(&cfg);
    if !refused.is_empty() {
        println!("\nCustom hooks refused (allow_custom_commands = false or remote origin):");
        for r in refused {
            println!("  - {r}");
        }
    }
    0
}

fn run_apply(scope: Option<&str>, file: &str) -> i32 {
    let Some(cfg) = load_with_scope(file, scope) else {
        eprintln!("No [ai_hooks] section in {file}");
        return 0;
    };
    if cfg.is_empty() {
        eprintln!("Nothing to apply: no agents or no hooks configured.");
        return 0;
    }
    let started = Instant::now();
    telemetry::ai_hook_phase_started(
        cfg.unique_agents().len(),
        cfg.hooks.len(),
        scope_label(cfg.scope),
        false,
    );
    match apply(&cfg) {
        Ok(report) => {
            println!(
                "Applied {} hook(s) across {} agent(s).",
                report.total_applied(),
                report.successes.len()
            );
            for o in &report.successes {
                println!(
                    "  {:<13} {} ({} applied)",
                    o.agent,
                    o.path.display(),
                    o.applied
                );
                for w in &o.warnings {
                    println!("      warning: {w}");
                }
                telemetry::ai_hook_agent_applied(o.agent, o.applied, o.warnings.len(), &o.path);
            }
            for (target, e) in &report.failures {
                eprintln!("  {:<13} FAILED ({}): {}", target.slug(), e.kind(), e);
                telemetry::ai_hook_agent_failed(target.slug(), e.kind());
            }
            if !report.refused_custom.is_empty() {
                println!(
                    "\nRefused {} custom hook(s) (allow_custom_commands = false):",
                    report.refused_custom.len()
                );
                for r in &report.refused_custom {
                    println!("  - {r}");
                }
            }
            if !report.remote_refused_custom.is_empty() {
                println!(
                    "\nRefused {} custom hook(s) from remote-fetched config:",
                    report.remote_refused_custom.len()
                );
                for r in &report.remote_refused_custom {
                    println!("  - {r}");
                }
            }
            telemetry::ai_hook_custom_refused_summary(
                report.refused_custom.len(),
                report.remote_refused_custom.len(),
            );
            telemetry::ai_hook_phase_completed(
                report.total_applied(),
                report.agents_touched(),
                report.refused_custom.len(),
                report.remote_refused_custom.len(),
                report.failures.len(),
                started.elapsed(),
            );
            if report.has_failures() {
                crate::error_codes::HOOK_FAILED
            } else {
                0
            }
        }
        Err(e) => {
            eprintln!("ai-hooks apply failed: {e}");
            telemetry::ai_hook_agent_failed("global", e.kind());
            crate::error_codes::HOOK_FAILED
        }
    }
}

fn run_check(scope: Option<&str>, file: &str) -> i32 {
    let Some(cfg) = load_with_scope(file, scope) else {
        eprintln!("No [ai_hooks] section in {file}");
        return 0;
    };
    let outcomes = check(&cfg);
    let mut drift = false;
    let mut errors = false;
    let mut drifted_agents = 0usize;
    for r in &outcomes {
        match r {
            Ok(o) => {
                if o.is_clean() {
                    println!("  {:<13} {} OK", o.agent, o.path.display());
                } else {
                    drift = true;
                    drifted_agents += 1;
                    println!("  {:<13} {} DRIFT", o.agent, o.path.display());
                    for m in &o.missing {
                        println!("      missing: {m}");
                    }
                    for x in &o.extra_jarvy {
                        println!("      extra jarvy-managed: {x}");
                    }
                }
            }
            Err((agent, e)) => {
                errors = true;
                eprintln!("  {:<13} FAILED ({}): {}", agent.slug(), e.kind(), e);
                telemetry::ai_hook_agent_failed(agent.slug(), e.kind());
            }
        }
    }
    telemetry::ai_hook_check_completed(outcomes.len(), drifted_agents);
    if errors {
        crate::error_codes::HOOK_FAILED
    } else if drift {
        1
    } else {
        0
    }
}

fn run_remove(scope: Option<&str>, file: &str) -> i32 {
    let Some(cfg) = load_with_scope(file, scope) else {
        eprintln!("No [ai_hooks] section in {file}");
        return 0;
    };
    let report = remove(&cfg);
    for o in &report.successes {
        println!(
            "  {:<13} {} removed {}",
            o.agent,
            o.path.display(),
            o.removed
        );
    }
    for (target, e) in &report.failures {
        eprintln!("  {:<13} FAILED ({}): {}", target.slug(), e.kind(), e);
        telemetry::ai_hook_agent_failed(target.slug(), e.kind());
    }
    if !report.failures.is_empty() {
        crate::error_codes::HOOK_FAILED
    } else {
        0
    }
}

fn run_test(name: &str) -> i32 {
    let Some(hook) = library::find(name) else {
        eprintln!("Unknown library hook: {name}");
        eprintln!("Run `jarvy ai-hooks list --library` for the full list.");
        return 2;
    };
    println!("Library hook: {}", hook.name);
    println!("  Event:    {}", hook.event);
    println!("  Matcher:  {:?}", hook.matcher.unwrap_or("(any)"));
    println!("  Timeout:  {}ms", hook.timeout_ms);
    println!("\n--- bash ---\n{}", hook.bash);
    println!("--- powershell ---\n{}", hook.powershell);
    0
}

/// Load the `[ai_hooks]` section from `file` and apply the runtime
/// `scope` override. Returns `None` if the section is missing — callers
/// surface that as a 0-exit no-op.
fn load_with_scope(file: &str, scope: Option<&str>) -> Option<AiHooksConfig> {
    let body = fs::read_to_string(file).ok()?;
    let cfg: Config = toml::from_str(&body).ok()?;
    let mut ai = cfg.ai_hooks?;
    // CLI loads are always Local — `jarvy ai-hooks apply --from <url>`
    // is intentionally not supported. Use `jarvy setup --from` if you
    // want the remote path with its trust-boundary enforcement.
    ai.origin = ConfigOrigin::Local;
    if let Some(s) = scope_from_str(scope) {
        ai.scope = s;
    }
    Some(ai)
}

fn scope_from_str(s: Option<&str>) -> Option<HookScope> {
    match s {
        Some("user") => Some(HookScope::User),
        Some("project") => Some(HookScope::Project),
        _ => None,
    }
}

fn scope_label(scope: HookScope) -> &'static str {
    match scope {
        HookScope::User => "user",
        HookScope::Project => "project",
    }
}
