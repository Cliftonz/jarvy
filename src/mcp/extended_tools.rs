//! Extended MCP tools exposing Jarvy's broader feature surface.
//!
//! Phase 2 of the Jarvy MCP integration: beyond the tool-installer
//! family (list_tools, install_tool, ...), this module wraps the
//! subsystems an AI agent benefits from being able to introspect and
//! drive directly — AI hooks, MCP registration, drift detection, role
//! definitions, services, templates, config validation.
//!
//! Naming convention: every tool here is prefixed `jarvy_` so a
//! cross-server `tools/list` from the agent's perspective shows them
//! grouped with the existing surface.
//!
//! Safety model:
//! - Read-only tools (`*_list`, `*_check`, `*_status`, `*_show`,
//!   `validate_config`) have no rate limiting and run unconditionally.
//! - Mutating tools (`*_apply`, `services_start`, `templates_use`)
//!   default to `dry_run = true` and require confirmation when
//!   `dry_run = false`, mirroring the `install_tool` flow.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::{Value, json};

use crate::mcp::audit::AuditLog;
use crate::mcp::config::McpConfig;
use crate::mcp::error::{McpError, McpResult};
use crate::mcp::safety::RateLimiter;
use crate::mcp::safety::{
    ConfirmationResult, prompt_mutation_confirmation, resolve_within_workspace,
};
use crate::mcp::tools::McpToolDefinition;

/// Collaborators every mutating extended-tool handler needs in order to
/// enforce the MCP safety boundary (rate limit, audit, confirmation,
/// workspace path containment). Built once per `tools/call` request by
/// the dispatcher in `server.rs`.
pub struct MutationCtx<'a> {
    pub config: &'a McpConfig,
    pub rate_limiter: &'a RateLimiter,
    pub audit_log: &'a AuditLog,
    pub client_name: Option<&'a str>,
    pub workspace_root: PathBuf,
}

impl MutationCtx<'_> {
    pub fn workspace(&self) -> &Path {
        &self.workspace_root
    }
}

/// Whether the current process is a descendant of `jarvy wizard --apply`.
///
/// Read from `JARVY_WIZARD_SESSION` env var, which the wizard sets on
/// the spawned agent CLI. Cached in a `LazyLock<bool>` because the env
/// is process-stable and this check runs on every `gate_mutation`
/// call — re-reading a lock-guarded env var here previously showed up
/// in the wizard hot path.
///
/// **Test override.** The bypass code path has no unit-testable seam
/// otherwise — `std::env::set_var` is process-global and racy under
/// parallel tests. Test builds honor a `#[cfg(test)]` re-read of the
/// env var so the `serial_test`-guarded tests can toggle behavior
/// per case; release builds get the memoized fast path.
#[cfg(not(test))]
fn is_wizard_session() -> bool {
    use std::sync::LazyLock;
    static WIZARD_SESSION: LazyLock<bool> =
        LazyLock::new(|| std::env::var("JARVY_WIZARD_SESSION").as_deref() == Ok("1"));
    *WIZARD_SESSION
}

#[cfg(test)]
fn is_wizard_session() -> bool {
    std::env::var("JARVY_WIZARD_SESSION").as_deref() == Ok("1")
}

/// Runs the shared MCP mutation guard before any extended tool mutates
/// state. Mirrors `handle_install_tool`'s flow:
///
/// 1. Audit the request (pre-flight, regardless of outcome).
/// 2. Apply the install rate-limit bucket (general "mutating MCP call").
/// 3. Prompt for confirmation unless `require_confirmation = false` or
///    the user previously selected "always" via the auto-approve flow.
/// 4. Audit the operator's decision (cancel / approve / always).
///
/// Returns `Ok(())` once the call is authorized to proceed. Errors bubble
/// up as `tools/call` failures so the agent sees a clean denial.
pub fn gate_mutation(
    ctx: &MutationCtx<'_>,
    tool_name: &str,
    effect_summary: &str,
) -> McpResult<()> {
    // Audit the request itself before any rate-limit / confirmation
    // decisions. Even a denial leaves a trail showing what was asked.
    ctx.audit_log.log_mcp_mutation(
        ctx.client_name,
        tool_name,
        false,
        true,
        Some(effect_summary),
    );

    // Reuse the install rate-limit bucket for any non-read MCP call.
    ctx.rate_limiter.check_install_limit().inspect_err(|_| {
        ctx.audit_log.log_rate_limited(ctx.client_name, tool_name);
    })?;

    // Skip the confirmation prompt when the operator has either
    // disabled it globally or pre-approved with "always allow".
    let global_auto_approve = crate::init::initialize().mcp.auto_approve_installs;
    if !ctx.config.mcp.require_confirmation || global_auto_approve {
        return Ok(());
    }

    // Wizard-session bypass. `jarvy wizard --apply` is itself the
    // operator's explicit consent — it spawns the agent CLI with
    // stdin piped (used for the prompt body), so the TTY prompt at
    // `prompt_mutation_confirmation` has no way to read a "yes" and
    // would fail closed, blocking the wizard mid-flight. Setting
    // `JARVY_WIZARD_SESSION=1` on the agent spawn marks the
    // descendant MCP server process as wizard-driven; gate_mutation
    // skips the second-layer prompt for those calls only. Telemetry
    // fires so the bypass is auditable.
    //
    // Threat model: the env var can be forged by anything running as
    // the same user, but at that point the attacker already has
    // user-level code-exec, which is strictly stronger than tricking
    // the MCP gate. The bypass narrowly serves a usability gap
    // inside `jarvy wizard`, not a privilege boundary.
    //
    // Audit trail: the pre-flight `log_mcp_mutation` call at the top
    // of this function already records the request. The `Yes` and
    // `Always` confirmation arms record NO extra audit entry (they
    // only emit tracing events for the operator's decision). The
    // wizard-bypass arm follows that pattern — the tracing event
    // below carries the forensic detail. A previous version of this
    // arm double-logged `log_mcp_mutation`, producing indistinguishable
    // duplicate rows in the audit log — removed intentionally.
    if is_wizard_session() {
        if crate::observability::telemetry_gate::is_enabled() {
            let session_id = std::env::var("JARVY_WIZARD_SESSION_ID")
                .unwrap_or_else(|_| String::new());
            let client_name = ctx.client_name.unwrap_or("unknown");
            let expected_clients = ["claude-code", "codex"];
            let client_unexpected = ctx
                .client_name
                .map(|c| !expected_clients.contains(&c))
                .unwrap_or(true);
            tracing::info!(
                event = "mcp.mutation.wizard_bypass",
                tool = tool_name,
                client = client_name,
                client_unexpected = client_unexpected,
                workspace = %ctx.workspace().display(),
                effect = effect_summary,
                pid = std::process::id(),
                wizard_session_id = %session_id,
            );
            if client_unexpected {
                // Elevated log level: an unexpected MCP client
                // exercising the wizard bypass is a forensic signal —
                // legitimate wizard usage always presents as
                // claude-code or codex.
                tracing::warn!(
                    event = "mcp.mutation.wizard_bypass_unexpected_client",
                    tool = tool_name,
                    client = client_name,
                    workspace = %ctx.workspace().display(),
                    wizard_session_id = %session_id,
                );
            }
        }
        return Ok(());
    }

    match prompt_mutation_confirmation(tool_name, effect_summary, ctx.client_name)? {
        ConfirmationResult::Yes => Ok(()),
        ConfirmationResult::No => {
            ctx.audit_log.log_cancelled(ctx.client_name, tool_name);
            Err(McpError::user_cancelled())
        }
        ConfirmationResult::Always => {
            // Persist "always allow" to ~/.jarvy/config.toml so a fleet
            // operator who said yes once doesn't get re-prompted.
            if let Err(e) = crate::init::modify_global_config(|cfg| {
                cfg.mcp.auto_approve_installs = true;
            }) {
                tracing::warn!(
                    event = "mcp.auto_approve.persist_failed",
                    tool = %tool_name,
                    error = %e,
                );
            } else {
                tracing::info!(
                    event = "mcp.auto_approve.enabled",
                    tool = %tool_name,
                    client = ctx.client_name.unwrap_or("unknown"),
                );
            }
            Ok(())
        }
    }
}

/// Tool definitions appended to the main `list_tools()` registration.
pub fn extended_definitions() -> Vec<McpToolDefinition> {
    vec![
        // ---- AI hooks --------------------------------------------------
        def(
            "jarvy_ai_hooks_list",
            "List configured AI hooks in jarvy.toml and the curated built-in library. Use this to understand what guardrails Jarvy can ship to AI coding agents.",
            json!({
                "type": "object",
                "properties": {
                    "config_path": { "type": "string", "description": "Path to jarvy.toml (default: ./jarvy.toml)" },
                    "library": { "type": "boolean", "description": "Show built-in library instead of project config" }
                }
            }),
        ),
        def(
            "jarvy_ai_hooks_check",
            "Detect drift between configured AI hooks and what is currently provisioned in each agent's settings file. Returns per-agent missing + extra-jarvy lists.",
            json!({
                "type": "object",
                "properties": {
                    "config_path": { "type": "string" }
                }
            }),
        ),
        def(
            "jarvy_ai_hooks_apply",
            "Apply the AI hooks configuration. Defaults to dry_run = true so the agent can preview what would change. Set dry_run = false to actually write the agent settings files.",
            json!({
                "type": "object",
                "properties": {
                    "config_path": { "type": "string" },
                    "dry_run": { "type": "boolean", "description": "Preview only (default true)" }
                }
            }),
        ),
        // ---- MCP server registration ---------------------------------
        def(
            "jarvy_mcp_register_list",
            "List MCP servers Jarvy is configured to register with AI agents. Includes the always-on jarvy entry plus any allow-listed custom servers.",
            json!({
                "type": "object",
                "properties": {
                    "config_path": { "type": "string" }
                }
            }),
        ),
        def(
            "jarvy_mcp_register_check",
            "Detect drift between configured MCP server registrations and each agent's on-disk config file.",
            json!({
                "type": "object",
                "properties": {
                    "config_path": { "type": "string" }
                }
            }),
        ),
        def(
            "jarvy_mcp_register_apply",
            "Apply MCP server registrations to every configured agent. Defaults to dry_run = true.",
            json!({
                "type": "object",
                "properties": {
                    "config_path": { "type": "string" },
                    "dry_run": { "type": "boolean" }
                }
            }),
        ),
        // ---- Drift -----------------------------------------------------
        def(
            "jarvy_drift_check",
            "Detect configuration drift in the current project — installed tool versions vs the jarvy.toml baseline state.",
            json!({
                "type": "object",
                "properties": {
                    "project_dir": { "type": "string", "description": "Path to the project root (default: cwd)" }
                }
            }),
        ),
        def(
            "jarvy_drift_status",
            "Show the current drift baseline state file (tools tracked, file hashes, last update).",
            json!({
                "type": "object",
                "properties": {
                    "project_dir": { "type": "string" }
                }
            }),
        ),
        // ---- Roles -----------------------------------------------------
        def(
            "jarvy_roles_list",
            "List roles defined in jarvy.toml. Each role bundles a set of tools so heterogeneous teams (frontend, devops, data) can share one config.",
            json!({
                "type": "object",
                "properties": {
                    "config_path": { "type": "string" }
                }
            }),
        ),
        def(
            "jarvy_roles_show",
            "Show full details for a specific role, including tools, inherited parents, and resolved tool list.",
            json!({
                "type": "object",
                "properties": {
                    "config_path": { "type": "string" },
                    "name": { "type": "string", "description": "Role name (e.g. 'frontend')" }
                },
                "required": ["name"]
            }),
        ),
        // ---- Services -------------------------------------------------
        def(
            "jarvy_services_status",
            "Check whether project services (docker-compose, Tilt) are running and which backend is active.",
            json!({
                "type": "object",
                "properties": {
                    "project_dir": { "type": "string" }
                }
            }),
        ),
        def(
            "jarvy_services_start",
            "Start project services (docker-compose up / tilt up). Defaults to dry_run = true; preview prints what would run. Pass detach = false to run in the foreground (attached).",
            json!({
                "type": "object",
                "properties": {
                    "project_dir": { "type": "string" },
                    "dry_run": { "type": "boolean", "description": "Preview only (default true)" },
                    "detach": { "type": "boolean", "description": "Run detached / in background (default true)" }
                }
            }),
        ),
        // ---- Templates ------------------------------------------------
        def(
            "jarvy_templates_list",
            "List built-in jarvy.toml templates (node-bun, python-uv, k8s-platform, etc.) — useful for scaffolding new projects.",
            json!({
                "type": "object",
                "properties": {
                    "category": { "type": "string", "description": "Optional category filter" }
                }
            }),
        ),
        def(
            "jarvy_templates_show",
            "Show full details for a specific built-in template — tools, hooks, env vars, description.",
            json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string" }
                },
                "required": ["name"]
            }),
        ),
        def(
            "jarvy_templates_use",
            "Scaffold a jarvy.toml from a built-in template. Defaults to dry_run = true; preview returns the would-be content. Set dry_run = false to write to disk (refuses to overwrite an existing file unless force = true).",
            json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Template name (run jarvy_templates_list to discover)" },
                    "output_path": { "type": "string", "description": "Where to write (default ./jarvy.toml)" },
                    "dry_run": { "type": "boolean" },
                    "force": { "type": "boolean", "description": "Overwrite an existing file (default false)" }
                },
                "required": ["name"]
            }),
        ),
        // ---- Config validation ----------------------------------------
        def(
            "jarvy_validate_config",
            "Parse and validate jarvy.toml. Returns the structured error list when the file is malformed or refers to unknown tools.",
            json!({
                "type": "object",
                "properties": {
                    "config_path": { "type": "string" }
                }
            }),
        ),
        // ---- Discover (PRD-044) ---------------------------------------
        def(
            "jarvy_discover_scan",
            "Scan the project directory for marker files (Cargo.toml, package.json, Dockerfile, k8s/, …) and return suggested tools. Read-only. Use jarvy_discover_apply to actually write to jarvy.toml.",
            json!({
                "type": "object",
                "properties": {
                    "project_dir": { "type": "string", "description": "Path to the project root (default: cwd / workspace root)" },
                    "config_path": { "type": "string", "description": "Path to jarvy.toml (used to dedupe against already-pinned tools; default ./jarvy.toml)" }
                }
            }),
        ),
        def(
            "jarvy_discover_apply",
            "Run discover and merge suggested tools into jarvy.toml. Append-only — hand-pinned tools survive untouched. Defaults to dry_run = true.",
            json!({
                "type": "object",
                "properties": {
                    "config_path": { "type": "string", "description": "Path to jarvy.toml (default ./jarvy.toml)" },
                    "dry_run": { "type": "boolean", "description": "Preview only (default true)" }
                }
            }),
        ),
        // ---- Workspace (PRD-047) --------------------------------------
        def(
            "jarvy_workspace_list",
            "Enumerate workspace members declared in [workspace] members and their resolved tool sets. Read-only.",
            json!({
                "type": "object",
                "properties": {
                    "config_path": { "type": "string", "description": "Path to root jarvy.toml (default ./jarvy.toml)" }
                }
            }),
        ),
        def(
            "jarvy_workspace_show",
            "Show one workspace member's resolved config with inheritance / override annotations.",
            json!({
                "type": "object",
                "properties": {
                    "config_path": { "type": "string" },
                    "name": { "type": "string", "description": "Member name as declared in [workspace] members" }
                },
                "required": ["name"]
            }),
        ),
        def(
            "jarvy_workspace_validate",
            "Validate that every workspace member exists and its jarvy.toml parses. Returns errors / warnings / refused-members.",
            json!({
                "type": "object",
                "properties": {
                    "config_path": { "type": "string" }
                }
            }),
        ),
        // ---- Library cache (PRD-054 phase 6) -------------------------
        def(
            "jarvy_library_list",
            "List every library currently in the process cache (URL, publisher, ai_hook/mcp_server/skill counts).",
            json!({"type": "object", "properties": {}}),
        ),
        def(
            "jarvy_library_show",
            "Show items inside one cached library (by URL).",
            json!({
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Library URL as declared in [<subsystem>.library_sources]" }
                },
                "required": ["url"]
            }),
        ),
        // ---- Wizard (PRD-056) ----------------------------------------
        def(
            "jarvy_wizard_plan",
            "Produce the agent-driven setup plan for the current project: discover detections, required / recommended tools, and a greenfield-vs-refinement flag. Read-only — the agent uses this to present a plan before calling jarvy_discover_apply / jarvy_ai_hooks_apply / etc.",
            json!({
                "type": "object",
                "properties": {
                    "project_dir": { "type": "string", "description": "Path to the project root (default: cwd / workspace root)" },
                    "config_path": { "type": "string", "description": "Path to jarvy.toml — used to set the greenfield flag and dedupe (default ./jarvy.toml)" }
                }
            }),
        ),
    ]
}

fn def(name: &str, description: &str, schema: Value) -> McpToolDefinition {
    McpToolDefinition {
        name: name.to_string(),
        description: description.to_string(),
        input_schema: schema,
    }
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// Wrap a JSON value into the MCP tool-call response envelope.
fn envelope(value: Value) -> McpResult<Value> {
    Ok(json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(&value)?
        }]
    }))
}

#[derive(Deserialize, Default)]
struct PathArgs {
    #[serde(default)]
    config_path: Option<String>,
    #[serde(default)]
    project_dir: Option<String>,
}

fn config_path(args: &PathArgs) -> String {
    args.config_path
        .clone()
        .unwrap_or_else(|| crate::cli::DEFAULT_CONFIG_FILE.to_string())
}

fn project_dir(args: &PathArgs) -> PathBuf {
    args.project_dir
        .clone()
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

fn parse<P: Default + serde::de::DeserializeOwned>(arguments: Option<Value>) -> McpResult<P> {
    Ok(arguments
        .map(serde_json::from_value)
        .transpose()?
        .unwrap_or_default())
}

// ---- AI hooks --------------------------------------------------------------

#[derive(Deserialize, Default)]
struct AiHooksListArgs {
    #[serde(default)]
    config_path: Option<String>,
    #[serde(default)]
    library: bool,
}

pub fn handle_ai_hooks_list(arguments: Option<Value>) -> McpResult<Value> {
    let args: AiHooksListArgs = parse(arguments)?;
    if args.library {
        let entries: Vec<Value> = crate::ai_hooks::library::LIBRARY
            .iter()
            .map(|h| {
                json!({
                    "name": h.name,
                    "description": h.description,
                    "event": h.event.to_string(),
                    "matcher": h.matcher,
                    "timeout_ms": h.timeout_ms,
                })
            })
            .collect();
        return envelope(json!({ "library": entries, "count": entries.len() }));
    }
    let file = args
        .config_path
        .unwrap_or_else(|| crate::cli::DEFAULT_CONFIG_FILE.to_string());
    let Some(cfg) = load_ai_hooks(&file) else {
        return envelope(json!({
            "configured": false,
            "config_path": file,
            "message": "No [ai_hooks] section in config"
        }));
    };
    let refused = crate::ai_hooks::runner::audit_custom_commands(&cfg);
    let hooks: Vec<Value> = cfg
        .hooks
        .iter()
        .map(|h| {
            json!({
                "identifier": h.identifier(),
                "kind": if h.is_library() { "library" } else if h.is_custom_command() { "custom" } else { "invalid" },
            })
        })
        .collect();
    envelope(json!({
        "configured": true,
        "config_path": file,
        "agents": cfg.unique_agents().iter().map(|a| a.slug()).collect::<Vec<_>>(),
        "scope": format!("{:?}", cfg.scope),
        "allow_custom_commands": cfg.allow_custom_commands,
        "origin": format!("{:?}", cfg.origin),
        "hooks": hooks,
        "refused_custom": refused,
    }))
}

pub fn handle_ai_hooks_check(arguments: Option<Value>) -> McpResult<Value> {
    let args: PathArgs = parse(arguments)?;
    let file = config_path(&args);
    let Some(cfg) = load_ai_hooks(&file) else {
        return envelope(json!({ "configured": false, "config_path": file }));
    };
    let outcomes = crate::ai_hooks::check(&cfg);
    let mut report = Vec::with_capacity(outcomes.len());
    let mut drifted = 0usize;
    let mut errored = 0usize;
    for r in outcomes {
        match r {
            Ok(o) => {
                if !o.is_clean() {
                    drifted += 1;
                }
                report.push(json!({
                    "agent": o.agent,
                    "path": o.path.display().to_string(),
                    "clean": o.is_clean(),
                    "missing": o.missing,
                    "extra_jarvy": o.extra_jarvy,
                }));
            }
            Err((agent, e)) => {
                errored += 1;
                report.push(json!({
                    "agent": agent.slug(),
                    "error_type": e.kind(),
                }));
            }
        }
    }
    envelope(json!({
        "configured": true,
        "config_path": file,
        "agents_checked": report.len(),
        "drifted": drifted,
        "errored": errored,
        "report": report,
    }))
}

#[derive(Deserialize, Default)]
struct ApplyArgs {
    #[serde(default)]
    config_path: Option<String>,
    #[serde(default)]
    dry_run: Option<bool>,
}

pub fn handle_ai_hooks_apply(arguments: Option<Value>, ctx: &MutationCtx<'_>) -> McpResult<Value> {
    let args: ApplyArgs = parse(arguments)?;
    let file = args
        .config_path
        .unwrap_or_else(|| crate::cli::DEFAULT_CONFIG_FILE.to_string());
    let Some(cfg) = load_ai_hooks(&file) else {
        return envelope(json!({ "configured": false, "config_path": file }));
    };
    let dry_run = args.dry_run.unwrap_or(true);
    if dry_run {
        let refused = crate::ai_hooks::runner::audit_custom_commands(&cfg);
        ctx.audit_log.log_mcp_mutation(
            ctx.client_name,
            "jarvy_ai_hooks_apply",
            true,
            true,
            Some(&format!(
                "preview: {} hook(s) across {} agent(s)",
                cfg.hooks.len(),
                cfg.unique_agents().len()
            )),
        );
        return envelope(json!({
            "dry_run": true,
            "would_apply_hooks": cfg.hooks.len(),
            "would_target_agents": cfg.unique_agents().iter().map(|a| a.slug()).collect::<Vec<_>>(),
            "would_refuse_custom": refused,
            "notes": "Set dry_run to false to actually write agent settings files. Mutating changes go through the host's stderr confirmation flow.",
        }));
    }
    let summary = format!(
        "Write {} hook(s) into the settings file of {} AI agent(s): {}",
        cfg.hooks.len(),
        cfg.unique_agents().len(),
        cfg.unique_agents()
            .iter()
            .map(|a| a.slug())
            .collect::<Vec<_>>()
            .join(", ")
    );
    gate_mutation(ctx, "jarvy_ai_hooks_apply", &summary)?;
    match crate::ai_hooks::apply(&cfg) {
        Ok(report) => envelope(json!({
            "dry_run": false,
            "applied": report.total_applied(),
            "agents_touched": report.agents_touched(),
            "successes": report.successes.iter().map(|o| json!({
                "agent": o.agent,
                "path": o.path.display().to_string(),
                "applied": o.applied,
            })).collect::<Vec<_>>(),
            "failures": report.failures.iter().map(|(t, e)| json!({
                "agent": t.slug(),
                "error_type": e.kind(),
            })).collect::<Vec<_>>(),
            "refused_custom": report.refused_custom,
            "remote_refused": report.remote_refused_custom,
        })),
        Err(e) => Err(McpError::internal_error(format!(
            "ai_hooks::apply failed ({}): {e}",
            e.kind()
        ))),
    }
}

fn load_ai_hooks(file: &str) -> Option<crate::ai_hooks::AiHooksConfig> {
    let body = std::fs::read_to_string(file).ok()?;
    let cfg: crate::config::Config = toml::from_str(&body).ok()?;
    let mut ai = cfg.ai_hooks?;
    ai.origin = crate::ai_hooks::ConfigOrigin::Local;
    Some(ai)
}

// ---- MCP register ----------------------------------------------------------

pub fn handle_mcp_register_list(arguments: Option<Value>) -> McpResult<Value> {
    let args: PathArgs = parse(arguments)?;
    let file = config_path(&args);
    let Some(cfg) = load_mcp_register(&file) else {
        return envelope(json!({ "configured": false, "config_path": file }));
    };
    let refused = crate::mcp_register::runner::audit_custom_servers(&cfg);
    envelope(json!({
        "configured": true,
        "config_path": file,
        "agents": cfg.unique_agents().iter().map(|a| a.slug()).collect::<Vec<_>>(),
        "scope": format!("{:?}", cfg.scope),
        "allow_custom_servers": cfg.allow_custom_servers,
        "origin": format!("{:?}", cfg.origin),
        "jarvy_server": "built-in (always registered)",
        "custom_servers": cfg.servers.iter().map(|s| json!({
            "name": s.name,
            "transport": format!("{:?}", s.transport),
        })).collect::<Vec<_>>(),
        "refused_custom": refused,
    }))
}

pub fn handle_mcp_register_check(arguments: Option<Value>) -> McpResult<Value> {
    let args: PathArgs = parse(arguments)?;
    let file = config_path(&args);
    let Some(cfg) = load_mcp_register(&file) else {
        return envelope(json!({ "configured": false, "config_path": file }));
    };
    let outcomes = crate::mcp_register::check(&cfg);
    let mut report = Vec::with_capacity(outcomes.len());
    let mut drifted = 0usize;
    let mut errored = 0usize;
    for r in outcomes {
        match r {
            Ok(o) => {
                if !o.is_clean() {
                    drifted += 1;
                }
                report.push(json!({
                    "agent": o.agent,
                    "path": o.path.display().to_string(),
                    "clean": o.is_clean(),
                    "missing": o.missing,
                    "extra_jarvy": o.extra_jarvy,
                }));
            }
            Err((agent, e)) => {
                errored += 1;
                report.push(json!({ "agent": agent.slug(), "error_type": e.kind() }));
            }
        }
    }
    envelope(json!({
        "configured": true,
        "config_path": file,
        "agents_checked": report.len(),
        "drifted": drifted,
        "errored": errored,
        "report": report,
    }))
}

pub fn handle_mcp_register_apply(
    arguments: Option<Value>,
    ctx: &MutationCtx<'_>,
) -> McpResult<Value> {
    let args: ApplyArgs = parse(arguments)?;
    let file = args
        .config_path
        .unwrap_or_else(|| crate::cli::DEFAULT_CONFIG_FILE.to_string());
    let Some(cfg) = load_mcp_register(&file) else {
        return envelope(json!({ "configured": false, "config_path": file }));
    };
    let dry_run = args.dry_run.unwrap_or(true);
    if dry_run {
        ctx.audit_log.log_mcp_mutation(
            ctx.client_name,
            "jarvy_mcp_register_apply",
            true,
            true,
            Some(&format!(
                "preview: {} server(s) across {} agent(s)",
                cfg.servers.len() + 1,
                cfg.unique_agents().len()
            )),
        );
        return envelope(json!({
            "dry_run": true,
            "would_register_servers": cfg.servers.len() + 1,
            "would_target_agents": cfg.unique_agents().iter().map(|a| a.slug()).collect::<Vec<_>>(),
            "notes": "Set dry_run to false to actually write agent MCP config files.",
        }));
    }
    let summary = format!(
        "Register {} MCP server(s) (jarvy + custom) with {} agent(s): {}",
        cfg.servers.len() + 1,
        cfg.unique_agents().len(),
        cfg.unique_agents()
            .iter()
            .map(|a| a.slug())
            .collect::<Vec<_>>()
            .join(", ")
    );
    gate_mutation(ctx, "jarvy_mcp_register_apply", &summary)?;
    match crate::mcp_register::apply(&cfg) {
        Ok(report) => envelope(json!({
            "dry_run": false,
            "applied": report.total_applied(),
            "agents_touched": report.agents_touched(),
            "successes": report.successes.iter().map(|o| json!({
                "agent": o.agent,
                "path": o.path.display().to_string(),
                "applied": o.applied,
            })).collect::<Vec<_>>(),
            "failures": report.failures.iter().map(|(t, e)| json!({
                "agent": t.slug(),
                "error_type": e.kind(),
            })).collect::<Vec<_>>(),
            "refused_custom": report.refused_custom,
            "remote_refused": report.remote_refused,
        })),
        Err(e) => Err(McpError::internal_error(format!(
            "mcp_register::apply failed ({}): {e}",
            e.kind()
        ))),
    }
}

fn load_mcp_register(file: &str) -> Option<crate::mcp_register::McpRegisterConfig> {
    let body = std::fs::read_to_string(file).ok()?;
    let cfg: crate::config::Config = toml::from_str(&body).ok()?;
    let mut mcp = cfg.mcp_register?;
    mcp.origin = crate::ai_hooks::ConfigOrigin::Local;
    Some(mcp)
}

// ---- Drift -----------------------------------------------------------------

pub fn handle_drift_check(arguments: Option<Value>) -> McpResult<Value> {
    let args: PathArgs = parse(arguments)?;
    let dir = project_dir(&args);
    let state_path = crate::paths::state_json(&dir);
    if !state_path.exists() {
        return envelope(json!({
            "baseline_exists": false,
            "project_dir": dir.display().to_string(),
            "message": "No drift baseline at .jarvy/state.json. Run `jarvy setup` first to capture one.",
        }));
    }
    // Read state, compare to current tool inventory. We surface the raw
    // baseline tool count + a sample so the agent can decide whether to
    // shell out to `jarvy drift check` for a full report.
    match crate::drift::state::EnvironmentState::load(&dir) {
        Ok(Some(state)) => envelope(json!({
            "baseline_exists": true,
            "project_dir": dir.display().to_string(),
            "tool_count": state.tool_count(),
            "files_tracked": state.file_count(),
            "notes": "Run `jarvy drift check` for the full per-tool comparison.",
        })),
        Ok(None) => envelope(json!({ "baseline_exists": false })),
        Err(e) => Err(McpError::internal_error(format!(
            "drift state load failed: {e}"
        ))),
    }
}

pub fn handle_drift_status(arguments: Option<Value>) -> McpResult<Value> {
    let args: PathArgs = parse(arguments)?;
    let dir = project_dir(&args);
    match crate::drift::state::EnvironmentState::load(&dir) {
        Ok(Some(state)) => envelope(json!({
            "baseline_exists": true,
            "project_dir": dir.display().to_string(),
            "tool_count": state.tool_count(),
            "files_tracked": state.file_count(),
        })),
        Ok(None) => envelope(json!({
            "baseline_exists": false,
            "project_dir": dir.display().to_string(),
        })),
        Err(e) => Err(McpError::internal_error(format!(
            "drift status load failed: {e}"
        ))),
    }
}

// ---- Roles -----------------------------------------------------------------

pub fn handle_roles_list(arguments: Option<Value>) -> McpResult<Value> {
    let args: PathArgs = parse(arguments)?;
    let file = config_path(&args);
    let Some(roles) = load_roles(&file) else {
        return envelope(json!({ "configured": false, "config_path": file }));
    };
    let entries: Vec<Value> = roles
        .iter()
        .map(|(name, def)| {
            json!({
                "name": name,
                "description": def.description,
                "extends": def.get_extends(),
                "tool_count": def.tool_count(),
            })
        })
        .collect();
    envelope(json!({
        "configured": true,
        "config_path": file,
        "count": entries.len(),
        "roles": entries,
    }))
}

#[derive(Deserialize)]
struct RolesShowArgs {
    name: String,
    #[serde(default)]
    config_path: Option<String>,
}

pub fn handle_roles_show(arguments: Option<Value>) -> McpResult<Value> {
    let args: RolesShowArgs = arguments
        .ok_or_else(|| McpError::invalid_params("Missing role name"))
        .and_then(|v| serde_json::from_value(v).map_err(McpError::from))?;
    let file = args
        .config_path
        .unwrap_or_else(|| crate::cli::DEFAULT_CONFIG_FILE.to_string());
    let Some(roles) = load_roles(&file) else {
        return envelope(json!({ "configured": false, "config_path": file }));
    };
    let Some(def) = roles.get(&args.name) else {
        return Err(McpError::invalid_params(format!(
            "Unknown role: {}",
            args.name
        )));
    };
    envelope(json!({
        "name": args.name,
        "description": def.description,
        "extends": def.get_extends(),
        "tools": def.get_tools(),
        "tool_count": def.tool_count(),
    }))
}

fn load_roles(
    file: &str,
) -> Option<std::collections::HashMap<String, crate::roles::definition::RoleDefinition>> {
    let body = std::fs::read_to_string(file).ok()?;
    let cfg: crate::config::Config = toml::from_str(&body).ok()?;
    let mut out = std::collections::HashMap::new();
    for (name, raw) in cfg.roles_config.roles.into_iter() {
        out.insert(name, raw.into_definition());
    }
    if out.is_empty() { None } else { Some(out) }
}

// ---- Services --------------------------------------------------------------

pub fn handle_services_status(arguments: Option<Value>) -> McpResult<Value> {
    let args: PathArgs = parse(arguments)?;
    let dir = project_dir(&args);
    let Some((backend, config_path)) = crate::services::detect_backend(&dir) else {
        return envelope(json!({
            "backend": null,
            "project_dir": dir.display().to_string(),
            "message": "No service backend detected (no docker-compose.yml / Tiltfile in project).",
        }));
    };
    let backend_impl = crate::services::get_backend(backend);
    envelope(json!({
        "backend": format!("{:?}", backend),
        "config_path": config_path.display().to_string(),
        "installed": backend_impl.is_installed(),
        "project_dir": dir.display().to_string(),
    }))
}

#[derive(Deserialize, Default)]
struct ServicesStartArgs {
    #[serde(default)]
    project_dir: Option<String>,
    #[serde(default)]
    dry_run: Option<bool>,
    #[serde(default)]
    detach: Option<bool>,
}

pub fn handle_services_start(arguments: Option<Value>, ctx: &MutationCtx<'_>) -> McpResult<Value> {
    let args: ServicesStartArgs = parse(arguments)?;
    // Caller-supplied project_dir is resolved RELATIVE to the MCP
    // workspace root. An absolute path or a `..` traversal that would
    // escape the workspace is refused — otherwise a malicious agent
    // could `services_start { project_dir: "/etc" }` and spin up
    // whatever docker-compose file happens to live there.
    let requested = args
        .project_dir
        .clone()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let dir = resolve_within_workspace(ctx.workspace(), &requested)?;
    let dry_run = args.dry_run.unwrap_or(true);
    let detach = args.detach.unwrap_or(true);
    let Some((backend, config_path)) = crate::services::detect_backend(&dir) else {
        return envelope(json!({
            "started": false,
            "project_dir": dir.display().to_string(),
            "message": "No docker-compose / Tilt config detected — nothing to start.",
        }));
    };
    let backend_impl = crate::services::get_backend(backend);
    if !backend_impl.is_installed() {
        return envelope(json!({
            "started": false,
            "backend": format!("{:?}", backend),
            "config_path": config_path.display().to_string(),
            "installed": false,
            "message": "Backend is not installed on this machine; run `jarvy setup` first.",
        }));
    }
    if dry_run {
        ctx.audit_log.log_mcp_mutation(
            ctx.client_name,
            "jarvy_services_start",
            true,
            true,
            Some(&format!(
                "preview: {backend:?} {} from {}",
                if detach { "(detached)" } else { "(attached)" },
                config_path.display()
            )),
        );
        return envelope(json!({
            "dry_run": true,
            "backend": format!("{:?}", backend),
            "config_path": config_path.display().to_string(),
            "detach": detach,
            "notes": "Set dry_run to false to actually start. Mutating ops go through the host's stderr confirmation flow.",
        }));
    }
    let summary = format!(
        "Start the {backend:?} backend using {} (detach={detach})",
        config_path.display()
    );
    gate_mutation(ctx, "jarvy_services_start", &summary)?;
    match backend_impl.start(&config_path, detach) {
        Ok(result) => envelope(json!({
            "dry_run": false,
            "backend": format!("{:?}", result.backend),
            "config_path": config_path.display().to_string(),
            "success": result.success,
            "message": result.message,
        })),
        Err(e) => Err(McpError::internal_error(format!(
            "services::start failed: {e}"
        ))),
    }
}

// ---- Templates -------------------------------------------------------------

#[derive(Deserialize, Default)]
struct TemplatesListArgs {
    #[serde(default)]
    category: Option<String>,
}

pub fn handle_templates_list(arguments: Option<Value>) -> McpResult<Value> {
    let args: TemplatesListArgs = parse(arguments)?;
    let all = crate::templates::builtin::list_builtin_templates();
    let filtered: Vec<&crate::templates::builtin::BuiltinTemplate> = match args.category {
        Some(ref c) => all
            .iter()
            .filter(|t| t.category.eq_ignore_ascii_case(c))
            .collect(),
        None => all.iter().collect(),
    };
    let entries: Vec<Value> = filtered
        .iter()
        .map(|t| {
            json!({
                "name": t.name,
                "description": t.description,
                "category": t.category,
            })
        })
        .collect();
    envelope(json!({
        "count": entries.len(),
        "categories": crate::templates::builtin::all_categories(),
        "templates": entries,
    }))
}

#[derive(Deserialize)]
struct TemplatesShowArgs {
    name: String,
}

pub fn handle_templates_show(arguments: Option<Value>) -> McpResult<Value> {
    let args: TemplatesShowArgs = arguments
        .ok_or_else(|| McpError::invalid_params("Missing template name"))
        .and_then(|v| serde_json::from_value(v).map_err(McpError::from))?;
    let Some(template) = crate::templates::builtin::get_builtin_template(&args.name) else {
        return Err(McpError::invalid_params(format!(
            "Unknown template: {}",
            args.name
        )));
    };
    let full = template.to_template();
    envelope(json!({
        "name": template.name,
        "description": template.description,
        "category": template.category,
        "tools": full.tools.tools,
        "meta": full.template,
    }))
}

#[derive(Deserialize)]
struct TemplatesUseArgs {
    name: String,
    #[serde(default)]
    output_path: Option<String>,
    #[serde(default)]
    dry_run: Option<bool>,
    #[serde(default)]
    force: Option<bool>,
}

pub fn handle_templates_use(arguments: Option<Value>, ctx: &MutationCtx<'_>) -> McpResult<Value> {
    let args: TemplatesUseArgs = arguments
        .ok_or_else(|| McpError::invalid_params("Missing template name"))
        .and_then(|v| serde_json::from_value(v).map_err(McpError::from))?;
    let Some(template) = crate::templates::builtin::get_builtin_template(&args.name) else {
        return Err(McpError::invalid_params(format!(
            "Unknown template: {}",
            args.name
        )));
    };
    let dry_run = args.dry_run.unwrap_or(true);
    let force = args.force.unwrap_or(false);
    // Caller-supplied output_path is resolved RELATIVE to the MCP
    // workspace root. Absolute paths and `..` traversal outside the
    // workspace are refused — otherwise the agent could clobber files
    // anywhere the Jarvy process can write.
    let requested = args
        .output_path
        .clone()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("jarvy.toml"));
    let output = resolve_within_workspace(ctx.workspace(), &requested)?;
    let content = template.to_template().to_jarvy_toml();
    if dry_run {
        ctx.audit_log.log_mcp_mutation(
            ctx.client_name,
            "jarvy_templates_use",
            true,
            true,
            Some(&format!("preview: {} → {}", args.name, output.display())),
        );
        return envelope(json!({
            "dry_run": true,
            "template": args.name,
            "output_path": output.display().to_string(),
            "tool_count": template.tools.len(),
            "would_overwrite": output.exists(),
            "content_preview": content,
        }));
    }
    if output.exists() && !force {
        return envelope(json!({
            "dry_run": false,
            "created": false,
            "output_path": output.display().to_string(),
            "error": "file already exists; pass force = true to overwrite",
        }));
    }
    let summary = format!(
        "Scaffold {} ({} bytes) → {}",
        args.name,
        content.len(),
        output.display()
    );
    gate_mutation(ctx, "jarvy_templates_use", &summary)?;
    let (backup, backed_up) = write_template_atomic(&output, content.as_bytes(), force)?;
    envelope(json!({
        "dry_run": false,
        "created": true,
        "template": args.name,
        "output_path": output.display().to_string(),
        "tool_count": template.tools.len(),
        "bytes_written": content.len(),
        "backed_up": backed_up,
        "backup_path": backup.map(|p| p.display().to_string()),
    }))
}

/// Atomic-write a template to `path`, backing up any existing file when
/// `force = true`. Returns `(backup_path, backed_up)` — the optional
/// path to the `.bak` sibling and a flag indicating whether a backup
/// was actually written.
///
/// Steps:
/// 1. If the target exists and `force == true`, copy it to
///    `<path>.bak`. Pre-existing `.bak` is overwritten — operators
///    asked for force, they get one.
/// 2. Write the new content to `<path>.jarvy.tmp.<pid>.<nanos>` with
///    `O_CREAT|O_EXCL`, fsync, then rename. No torn file at `path`
///    even if the process is killed mid-write.
fn write_template_atomic(
    path: &Path,
    content: &[u8],
    force: bool,
) -> McpResult<(Option<PathBuf>, bool)> {
    use std::fs::{self, OpenOptions};
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};

    let mut backup_path: Option<PathBuf> = None;
    let mut backed_up = false;
    if path.exists() {
        if !force {
            return Err(McpError::invalid_params(format!(
                "file already exists at {}; pass force = true to overwrite",
                path.display()
            )));
        }
        let bak = path.with_extension({
            let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("toml");
            format!("{ext}.bak")
        });
        fs::copy(path, &bak).map_err(|e| {
            McpError::internal_error(format!(
                "failed to back up existing {}: {e}",
                path.display()
            ))
        })?;
        backup_path = Some(bak);
        backed_up = true;
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            McpError::internal_error(format!("failed to create parent {}: {e}", parent.display()))
        })?;
    }
    let pid = std::process::id();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp = path.with_extension(format!("jarvy.tmp.{pid}.{nanos}"));
    {
        let mut f = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&tmp)
            .map_err(|e| {
                McpError::internal_error(format!(
                    "failed to create tempfile {}: {e}",
                    tmp.display()
                ))
            })?;
        f.write_all(content)
            .map_err(|e| McpError::internal_error(format!("failed to write tempfile: {e}")))?;
        f.sync_all()
            .map_err(|e| McpError::internal_error(format!("failed to fsync tempfile: {e}")))?;
    }
    fs::rename(&tmp, path).map_err(|e| {
        McpError::internal_error(format!("failed to rename tempfile into place: {e}"))
    })?;
    Ok((backup_path, backed_up))
}

// ---- Config validation -----------------------------------------------------

pub fn handle_validate_config(arguments: Option<Value>) -> McpResult<Value> {
    let args: PathArgs = parse(arguments)?;
    let file = config_path(&args);
    let path = Path::new(&file);
    if !path.exists() {
        return envelope(json!({
            "valid": false,
            "config_path": file,
            "error_type": "missing",
            "message": format!("File not found: {file}"),
        }));
    }
    let body = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            return envelope(json!({
                "valid": false,
                "config_path": file,
                "error_type": "io",
                "message": e.to_string(),
            }));
        }
    };
    match toml::from_str::<crate::config::Config>(&body) {
        Ok(cfg) => envelope(json!({
            "valid": true,
            "config_path": file,
            "tool_count": cfg.tool_configs_len(),
            "has_ai_hooks": cfg.ai_hooks.is_some(),
            "has_mcp_register": cfg.mcp_register.is_some(),
            "has_git": cfg.git.is_some(),
            "has_npm": cfg.npm.is_some(),
            "has_pip": cfg.pip.is_some(),
            "has_cargo": cfg.cargo.is_some(),
            "has_drift": cfg.drift.is_some(),
        })),
        Err(e) => envelope(json!({
            "valid": false,
            "config_path": file,
            "error_type": "parse",
            "message": e.to_string(),
        })),
    }
}

// ---------------------------------------------------------------------
// Discover (PRD-044)
// ---------------------------------------------------------------------

#[derive(Deserialize, Default)]
struct DiscoverScanArgs {
    #[serde(default)]
    project_dir: Option<String>,
    #[serde(default)]
    config_path: Option<String>,
}

pub fn handle_discover_scan(arguments: Option<Value>) -> McpResult<Value> {
    let args: DiscoverScanArgs = arguments
        .map(serde_json::from_value)
        .transpose()
        .map_err(|e| McpError::invalid_params(e.to_string()))?
        .unwrap_or_default();
    let project_dir = std::path::PathBuf::from(args.project_dir.unwrap_or_else(|| ".".into()));
    let config_path = args
        .config_path
        .unwrap_or_else(|| crate::cli::DEFAULT_CONFIG_FILE.into());

    let existing_text = std::fs::read_to_string(&config_path).ok();
    let already_configured: std::collections::HashSet<String> = existing_text
        .as_deref()
        .and_then(|t| t.parse::<toml::Table>().ok())
        .and_then(|t| t.get("provisioner").and_then(|v| v.as_table()).cloned())
        .map(|t| t.keys().cloned().collect())
        .unwrap_or_default();

    crate::tools::register_all();
    let known_tools: std::collections::HashSet<String> =
        crate::tools::registry::registered_tool_names()
            .into_iter()
            .collect();
    let report = crate::discover::analyze(&project_dir, &already_configured, &known_tools);

    envelope(json!({
        "project_dir": project_dir.display().to_string(),
        "detections": report.detections,
        "required": report.required,
        "recommended": report.recommended,
        "already_configured": report.already_configured,
    }))
}

#[derive(Deserialize, Default)]
struct DiscoverApplyArgs {
    #[serde(default)]
    config_path: Option<String>,
    #[serde(default = "default_true_arg")]
    dry_run: bool,
}

fn default_true_arg() -> bool {
    true
}

pub fn handle_discover_apply(arguments: Option<Value>, ctx: &MutationCtx) -> McpResult<Value> {
    let args: DiscoverApplyArgs = arguments
        .map(serde_json::from_value)
        .transpose()
        .map_err(|e| McpError::invalid_params(e.to_string()))?
        .unwrap_or(DiscoverApplyArgs {
            config_path: None,
            dry_run: true,
        });
    let config_path = args
        .config_path
        .unwrap_or_else(|| crate::cli::DEFAULT_CONFIG_FILE.into());

    // Workspace containment — refuse a config_path that escapes the
    // mutation ctx's workspace root (consistent with templates_use
    // and ai_hooks_apply).
    let resolved = resolve_within_workspace(ctx.workspace(), std::path::Path::new(&config_path))?;

    if args.dry_run {
        // Preview without rate limit / confirmation — read-only.
        return handle_discover_scan(Some(json!({
            "config_path": resolved.display().to_string(),
        })));
    }

    // Mutating path: rate-limit + confirm + audit via the shared gate.
    gate_mutation(
        ctx,
        "jarvy_discover_apply",
        &format!("write suggested tools to {}", resolved.display()),
    )?;

    let exit = crate::discover::commands::run_discover(
        resolved.to_str().unwrap_or(&config_path),
        true,
        false,
        "json",
    );
    envelope(json!({
        "status": if exit == 0 { "applied" } else { "failed" },
        "exit_code": exit,
        "config_path": resolved.display().to_string(),
    }))
}

// ---------------------------------------------------------------------
// Workspace (PRD-047)
// ---------------------------------------------------------------------

#[derive(Deserialize, Default)]
struct WorkspaceListArgs {
    #[serde(default)]
    config_path: Option<String>,
}

pub fn handle_workspace_list(arguments: Option<Value>) -> McpResult<Value> {
    let args: WorkspaceListArgs = arguments
        .map(serde_json::from_value)
        .transpose()
        .map_err(|e| McpError::invalid_params(e.to_string()))?
        .unwrap_or_default();
    let config_path = args
        .config_path
        .unwrap_or_else(|| crate::cli::DEFAULT_CONFIG_FILE.into());
    let project_dir = crate::paths::config_parent_dir(&config_path);

    match crate::workspace::find_workspace_root(&project_dir) {
        Some(ctx) => {
            // Mirror the CLI shape — list of member summaries.
            envelope(json!({
                "workspace_root": ctx.root_config.parent().map(|p| p.display().to_string()),
                "inherit": ctx.workspace.effective_inherit(),
                "members": ctx.workspace.members,
            }))
        }
        None => envelope(json!({
            "status": "no_workspace",
            "searched_from": project_dir.display().to_string(),
        })),
    }
}

#[derive(Deserialize)]
struct WorkspaceShowArgs {
    #[serde(default)]
    config_path: Option<String>,
    name: String,
}

pub fn handle_workspace_show(arguments: Option<Value>) -> McpResult<Value> {
    let args: WorkspaceShowArgs = arguments
        .ok_or_else(|| McpError::invalid_params("name is required"))
        .and_then(|v| {
            serde_json::from_value(v).map_err(|e| McpError::invalid_params(e.to_string()))
        })?;
    let config_path = args
        .config_path
        .unwrap_or_else(|| crate::cli::DEFAULT_CONFIG_FILE.into());
    let project_dir = crate::paths::config_parent_dir(&config_path);

    let ctx = match crate::workspace::find_workspace_root(&project_dir) {
        Some(c) => c,
        None => {
            return envelope(json!({
                "status": "no_workspace",
                "searched_from": project_dir.display().to_string(),
            }));
        }
    };

    if !ctx.workspace.members.iter().any(|m| m == &args.name) {
        return envelope(json!({"status": "unknown_member", "name": args.name}));
    }

    envelope(json!({
        "workspace_root": ctx.root_config.parent().map(|p| p.display().to_string()),
        "member": args.name,
        "inherit": ctx.workspace.effective_inherit(),
    }))
}

#[derive(Deserialize, Default)]
struct WorkspaceValidateArgs {
    #[serde(default)]
    config_path: Option<String>,
}

pub fn handle_workspace_validate(arguments: Option<Value>) -> McpResult<Value> {
    let args: WorkspaceValidateArgs = arguments
        .map(serde_json::from_value)
        .transpose()
        .map_err(|e| McpError::invalid_params(e.to_string()))?
        .unwrap_or_default();
    let config_path = args
        .config_path
        .unwrap_or_else(|| crate::cli::DEFAULT_CONFIG_FILE.into());
    let project_dir = crate::paths::config_parent_dir(&config_path);

    let ctx = match crate::workspace::find_workspace_root(&project_dir) {
        Some(c) => c,
        None => {
            return envelope(json!({
                "status": "no_workspace",
                "searched_from": project_dir.display().to_string(),
            }));
        }
    };

    let root_dir = ctx
        .root_config
        .parent()
        .unwrap_or(std::path::Path::new("."));
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let mut entries = Vec::new();
    for member in &ctx.workspace.members {
        let p = std::path::Path::new(member);
        if p.is_absolute()
            || p.components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            errors.push(format!("{member}: escapes workspace root"));
            entries.push(json!({"name": member, "refused": true}));
            continue;
        }
        let member_dir = root_dir.join(member);
        let cfg = member_dir.join("jarvy.toml");
        let dir_exists = member_dir.is_dir();
        let cfg_exists = cfg.exists();
        let parses = if cfg_exists {
            std::fs::read_to_string(&cfg)
                .ok()
                .and_then(|s| toml::from_str::<toml::Value>(&s).ok())
                .is_some()
        } else {
            true
        };
        if !dir_exists {
            errors.push(format!("{member}: directory missing"));
        } else if !cfg_exists {
            warnings.push(format!("{member}: no jarvy.toml"));
        } else if !parses {
            errors.push(format!("{member}: jarvy.toml failed to parse"));
        }
        entries.push(json!({
            "name": member,
            "dir_exists": dir_exists,
            "config_exists": cfg_exists,
            "config_parses": parses,
        }));
    }
    let status = if !errors.is_empty() {
        "invalid"
    } else if !warnings.is_empty() {
        "warnings"
    } else {
        "ok"
    };
    envelope(json!({
        "status": status,
        "errors": errors,
        "warnings": warnings,
        "members": entries,
    }))
}

// ---------------------------------------------------------------------
// Library cache (PRD-054 phase 6)
// ---------------------------------------------------------------------

pub fn handle_library_list(_arguments: Option<Value>) -> McpResult<Value> {
    let libs = crate::library_registry::list_cached();
    envelope(json!({
        "count": libs.len(),
        "libraries": libs,
    }))
}

#[derive(Deserialize)]
struct LibraryShowArgs {
    url: String,
}

pub fn handle_library_show(arguments: Option<Value>) -> McpResult<Value> {
    let args: LibraryShowArgs = arguments
        .ok_or_else(|| McpError::invalid_params("url is required"))
        .and_then(|v| {
            serde_json::from_value(v).map_err(|e| McpError::invalid_params(e.to_string()))
        })?;
    match crate::library_registry::get_cached(&args.url) {
        Some((url, manifest)) => envelope(json!({
            "url": url,
            "publisher": manifest.publisher,
            "description": manifest.description,
            "items": manifest.items,
        })),
        None => envelope(json!({"status": "not_found", "url": args.url})),
    }
}

// ---------------------------------------------------------------------
// Wizard (PRD-056)
// ---------------------------------------------------------------------

#[derive(Deserialize, Default)]
struct WizardPlanArgs {
    #[serde(default)]
    project_dir: Option<String>,
    #[serde(default)]
    config_path: Option<String>,
}

/// Read-only plan generator for `jarvy wizard`. The agent calls this
/// to get a structured proposal — detections, required / recommended
/// tools, uninstallable bucket, plus a `greenfield` boolean — before
/// invoking any mutating tool (`jarvy_discover_apply`, etc.). Mirrors
/// the shape `wizard::context::ProjectContext` exposes to the headless
/// prompt so both modes stay consistent.
pub fn handle_wizard_plan(arguments: Option<Value>) -> McpResult<Value> {
    let args: WizardPlanArgs = arguments
        .map(serde_json::from_value)
        .transpose()
        .map_err(|e| McpError::invalid_params(e.to_string()))?
        .unwrap_or_default();
    let project_dir = std::path::PathBuf::from(args.project_dir.unwrap_or_else(|| ".".into()));
    let config_path = args
        .config_path
        .unwrap_or_else(|| crate::cli::DEFAULT_CONFIG_FILE.into());

    let existing_text = std::fs::read_to_string(&config_path).ok();
    let has_jarvy_toml = existing_text.is_some();
    let already_configured: std::collections::HashSet<String> = existing_text
        .as_deref()
        .and_then(|t| t.parse::<toml::Table>().ok())
        .and_then(|t| t.get("provisioner").and_then(|v| v.as_table()).cloned())
        .map(|t| t.keys().cloned().collect())
        .unwrap_or_default();

    crate::tools::register_all();
    let known_tools: std::collections::HashSet<String> =
        crate::tools::registry::registered_tool_names()
            .into_iter()
            .collect();
    let report = crate::discover::analyze(&project_dir, &already_configured, &known_tools);

    envelope(json!({
        "project_dir": project_dir.display().to_string(),
        "config_path": config_path,
        "has_jarvy_toml": has_jarvy_toml,
        "greenfield": !has_jarvy_toml,
        "detections": report.detections,
        "required": report.required,
        "recommended": report.recommended,
        "already_configured": report.already_configured,
        "uninstallable": report.uninstallable,
        "next_actions": [
            if has_jarvy_toml {
                "Present this plan to the user. If they confirm, call jarvy_discover_apply (merge mode) to add missing tools to [provisioner]."
            } else {
                "Greenfield: present this plan to the user. If they confirm, call jarvy_discover_apply with apply=true to bootstrap a starter jarvy.toml."
            },
            "Then call jarvy_validate_config to confirm the resulting jarvy.toml parses cleanly.",
            "Finally, remind the user to run `jarvy setup` themselves — don't run install commands from the wizard."
        ]
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::audit::AuditLog;
    use crate::mcp::config::McpConfig;
    use crate::mcp::safety::RateLimiter;
    use serde_json::json;
    use tempfile::TempDir;

    /// Configure an MCP context for tests: workspace pinned to `dir`,
    /// confirmation disabled so the prompt isn't reached, rate limiter
    /// reset.
    struct TestCtx {
        config: McpConfig,
        rate_limiter: RateLimiter,
        audit_log: AuditLog,
        workspace_root: std::path::PathBuf,
    }

    impl TestCtx {
        fn new(workspace: &std::path::Path) -> Self {
            let mut config = McpConfig::default();
            config.mcp.require_confirmation = false;
            let rate_limiter = RateLimiter::new(&config);
            let audit_log = AuditLog::disabled();
            Self {
                config,
                rate_limiter,
                audit_log,
                workspace_root: workspace.to_path_buf(),
            }
        }

        fn ctx(&self) -> MutationCtx<'_> {
            MutationCtx {
                config: &self.config,
                rate_limiter: &self.rate_limiter,
                audit_log: &self.audit_log,
                client_name: Some("jarvy-tests"),
                workspace_root: self.workspace_root.clone(),
            }
        }
    }

    #[test]
    fn ai_hooks_list_library_returns_curated_set() {
        let resp = handle_ai_hooks_list(Some(json!({ "library": true }))).unwrap();
        let text = resp["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("block-rm-rf"));
        assert!(text.contains("audit-log"));
    }

    #[test]
    fn ai_hooks_list_returns_not_configured_for_missing_file() {
        let resp =
            handle_ai_hooks_list(Some(json!({ "config_path": "/nonexistent.toml" }))).unwrap();
        let text = resp["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"configured\": false"));
    }

    #[test]
    fn validate_config_reports_missing_file() {
        let resp = handle_validate_config(Some(json!({ "config_path": "/nope.toml" }))).unwrap();
        let text = resp["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"valid\": false"));
        assert!(text.contains("missing"));
    }

    #[test]
    fn validate_config_parses_minimal_config() {
        let dir = TempDir::new().unwrap();
        let p = dir.path().join("jarvy.toml");
        std::fs::write(
            &p,
            r#"[provisioner]
git = "latest"
"#,
        )
        .unwrap();
        let resp = handle_validate_config(Some(json!({
            "config_path": p.to_str().unwrap()
        })))
        .unwrap();
        let text = resp["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"valid\": true"));
        assert!(text.contains("\"tool_count\": 1"));
    }

    #[test]
    fn templates_list_returns_built_in_templates() {
        let resp = handle_templates_list(None).unwrap();
        let text = resp["content"][0]["text"].as_str().unwrap();
        // At least one well-known template should be present.
        assert!(text.contains("templates"));
        assert!(text.contains("\"count\":"));
    }

    #[test]
    fn drift_status_reports_no_baseline_when_absent() {
        let dir = TempDir::new().unwrap();
        let resp = handle_drift_status(Some(json!({
            "project_dir": dir.path().to_str().unwrap()
        })))
        .unwrap();
        let text = resp["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"baseline_exists\": false"));
    }

    #[test]
    fn services_status_reports_no_backend_in_empty_dir() {
        let dir = TempDir::new().unwrap();
        let resp = handle_services_status(Some(json!({
            "project_dir": dir.path().to_str().unwrap()
        })))
        .unwrap();
        let text = resp["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"backend\": null"));
    }

    #[test]
    fn services_start_in_empty_dir_reports_not_started() {
        let dir = TempDir::new().unwrap();
        let tc = TestCtx::new(dir.path());
        // Relative path inside the workspace.
        let resp = handle_services_start(Some(json!({ "project_dir": "." })), &tc.ctx()).unwrap();
        let text = resp["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"started\": false"));
    }

    #[test]
    fn templates_use_dry_run_returns_preview_without_writing() {
        let dir = TempDir::new().unwrap();
        let tc = TestCtx::new(dir.path());
        let any_name = crate::templates::builtin::list_builtin_templates()
            .first()
            .map(|t| t.name.to_string())
            .expect("at least one built-in template");
        let resp = handle_templates_use(
            Some(json!({
                "name": any_name,
                "output_path": "jarvy.toml",
                "dry_run": true
            })),
            &tc.ctx(),
        )
        .unwrap();
        let text = resp["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"dry_run\": true"));
        assert!(text.contains("\"content_preview\""));
        assert!(!dir.path().join("jarvy.toml").exists());
    }

    #[test]
    fn templates_use_refuses_to_overwrite_without_force() {
        let dir = TempDir::new().unwrap();
        let tc = TestCtx::new(dir.path());
        let out = dir.path().join("jarvy.toml");
        std::fs::write(&out, b"existing").unwrap();
        let any_name = crate::templates::builtin::list_builtin_templates()
            .first()
            .map(|t| t.name.to_string())
            .unwrap();
        let resp = handle_templates_use(
            Some(json!({
                "name": any_name,
                "output_path": "jarvy.toml",
                "dry_run": false,
                "force": false
            })),
            &tc.ctx(),
        )
        .unwrap();
        let text = resp["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"created\": false"));
        assert!(text.contains("already exists"));
        assert_eq!(std::fs::read(&out).unwrap(), b"existing");
    }

    #[test]
    fn templates_use_writes_when_force_is_set_and_backs_up_existing() {
        let dir = TempDir::new().unwrap();
        let tc = TestCtx::new(dir.path());
        let out = dir.path().join("jarvy.toml");
        std::fs::write(&out, b"existing").unwrap();
        let any_name = crate::templates::builtin::list_builtin_templates()
            .first()
            .map(|t| t.name.to_string())
            .unwrap();
        let resp = handle_templates_use(
            Some(json!({
                "name": any_name,
                "output_path": "jarvy.toml",
                "dry_run": false,
                "force": true
            })),
            &tc.ctx(),
        )
        .unwrap();
        let text = resp["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"created\": true"));
        assert!(text.contains("\"backed_up\": true"));
        let body = std::fs::read_to_string(&out).unwrap();
        assert!(body.contains("[provisioner]") || body.contains("provisioner"));
        // Backup sibling preserves the pre-existing bytes.
        let bak = dir.path().join("jarvy.toml.bak");
        assert!(bak.exists(), "force should produce a .bak sibling");
        assert_eq!(std::fs::read(&bak).unwrap(), b"existing");
    }

    #[test]
    fn templates_use_unknown_template_returns_error() {
        let dir = TempDir::new().unwrap();
        let tc = TestCtx::new(dir.path());
        let resp = handle_templates_use(
            Some(json!({ "name": "definitely-not-a-real-template" })),
            &tc.ctx(),
        );
        let err = resp.unwrap_err();
        assert!(err.to_string().contains("Unknown template"));
    }

    // ---- Workspace path containment (Codex finding #2) ------------------

    #[test]
    fn templates_use_refuses_absolute_path_outside_workspace() {
        let dir = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        let tc = TestCtx::new(dir.path());
        let any_name = crate::templates::builtin::list_builtin_templates()
            .first()
            .map(|t| t.name.to_string())
            .unwrap();
        let attempt = outside.path().join("jarvy.toml");
        let err = handle_templates_use(
            Some(json!({
                "name": any_name,
                "output_path": attempt.to_str().unwrap(),
                "dry_run": false,
            })),
            &tc.ctx(),
        )
        .unwrap_err();
        assert!(
            err.to_string().to_lowercase().contains("workspace"),
            "expected workspace refusal, got: {err}"
        );
        assert!(!attempt.exists(), "must not have written outside workspace");
    }

    #[test]
    fn templates_use_refuses_parent_traversal() {
        let dir = TempDir::new().unwrap();
        let tc = TestCtx::new(dir.path());
        let any_name = crate::templates::builtin::list_builtin_templates()
            .first()
            .map(|t| t.name.to_string())
            .unwrap();
        let err = handle_templates_use(
            Some(json!({
                "name": any_name,
                "output_path": "../escape.toml",
                "dry_run": false,
            })),
            &tc.ctx(),
        )
        .unwrap_err();
        let s = err.to_string().to_lowercase();
        assert!(s.contains("workspace") || s.contains("escape"));
    }

    #[cfg(unix)]
    #[test]
    fn templates_use_refuses_to_write_through_symlink() {
        use std::os::unix::fs::symlink;
        let dir = TempDir::new().unwrap();
        let tc = TestCtx::new(dir.path());
        let target = dir.path().join("real.toml");
        std::fs::write(&target, b"real-content").unwrap();
        let link = dir.path().join("jarvy.toml");
        symlink(&target, &link).unwrap();
        let any_name = crate::templates::builtin::list_builtin_templates()
            .first()
            .map(|t| t.name.to_string())
            .unwrap();
        let err = handle_templates_use(
            Some(json!({
                "name": any_name,
                "output_path": "jarvy.toml",
                "dry_run": false,
                "force": true
            })),
            &tc.ctx(),
        )
        .unwrap_err();
        assert!(err.to_string().to_lowercase().contains("symlink"));
        // Real file untouched — the symlink defense ran BEFORE the
        // backup step.
        assert_eq!(std::fs::read(&target).unwrap(), b"real-content");
    }

    #[test]
    fn services_start_refuses_absolute_outside_workspace() {
        let dir = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        let tc = TestCtx::new(dir.path());
        let err = handle_services_start(
            Some(json!({ "project_dir": outside.path().to_str().unwrap(), "dry_run": false })),
            &tc.ctx(),
        )
        .unwrap_err();
        assert!(err.to_string().to_lowercase().contains("workspace"));
    }

    // ---- Mutation guard (Codex finding #1) ------------------------------

    #[test]
    fn ai_hooks_apply_dry_run_emits_audit_event_without_writing() {
        // The dry-run path doesn't trip the rate limiter or the
        // confirmation prompt, but it MUST emit an audit event so the
        // operator can see the agent previewed a mutation.
        let dir = TempDir::new().unwrap();
        let tc = TestCtx::new(dir.path());
        let f = dir.path().join("jarvy.toml");
        std::fs::write(
            &f,
            r#"[provisioner]
git = "latest"

[ai_hooks]
agents = ["claude-code"]

[[ai_hooks.hook]]
use = "block-rm-rf"
"#,
        )
        .unwrap();
        let resp = handle_ai_hooks_apply(
            Some(json!({ "config_path": f.to_str().unwrap(), "dry_run": true })),
            &tc.ctx(),
        )
        .unwrap();
        let text = resp["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"dry_run\": true"));
    }

    #[test]
    fn mcp_register_apply_dry_run_emits_audit_event() {
        let dir = TempDir::new().unwrap();
        let tc = TestCtx::new(dir.path());
        let f = dir.path().join("jarvy.toml");
        std::fs::write(
            &f,
            r#"[provisioner]
git = "latest"

[mcp_register]
agents = ["claude-code"]
"#,
        )
        .unwrap();
        let resp = handle_mcp_register_apply(
            Some(json!({ "config_path": f.to_str().unwrap(), "dry_run": true })),
            &tc.ctx(),
        )
        .unwrap();
        let text = resp["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"dry_run\": true"));
    }

    // ---------------------------------------------------------------
    // Wizard-session mutation-gate bypass. `JARVY_WIZARD_SESSION=1`
    // marks descendant MCP-server processes as wizard-driven so
    // `gate_mutation` can skip the TTY confirmation prompt (which
    // has nothing to read from — the agent's stdin holds the wizard
    // prompt body). See `is_wizard_session()` for the check.
    //
    // Env-var manipulation is process-global, so these tests are
    // `serial_test`-guarded and use a RAII guard to reset the env
    // even on assertion failure. The `#[cfg(test)]` branch of
    // `is_wizard_session` re-reads the env var on every call so
    // parallel tests don't see each other's setting.
    // ---------------------------------------------------------------

    /// RAII guard: unset `JARVY_WIZARD_SESSION` on drop so a panicking
    /// test cannot leak the env var into subsequent test cases.
    struct WizardEnvGuard;
    impl WizardEnvGuard {
        #[allow(unsafe_code)]
        fn set(value: &str) -> Self {
            // SAFETY: `serial_test::serial` around each caller prevents
            // parallel writes; env mutation is safe under that
            // guarantee.
            unsafe { std::env::set_var("JARVY_WIZARD_SESSION", value) };
            Self
        }
    }
    impl Drop for WizardEnvGuard {
        #[allow(unsafe_code)]
        fn drop(&mut self) {
            // SAFETY: same as `set` — serial_test guards ordering.
            unsafe { std::env::remove_var("JARVY_WIZARD_SESSION") };
        }
    }

    fn ctx_requiring_confirmation<'a>(tc: &'a mut TestCtx) -> MutationCtx<'a> {
        tc.config.mcp.require_confirmation = true;
        MutationCtx {
            config: &tc.config,
            rate_limiter: &tc.rate_limiter,
            audit_log: &tc.audit_log,
            client_name: Some("claude-code"),
            workspace_root: tc.workspace_root.clone(),
        }
    }

    #[test]
    #[serial_test::serial]
    fn wizard_bypass_skips_prompt_when_env_set_exactly_to_one() {
        let dir = TempDir::new().unwrap();
        let mut tc = TestCtx::new(dir.path());
        let _guard = WizardEnvGuard::set("1");
        // If the bypass fires, gate_mutation returns Ok(()) without
        // hitting `prompt_mutation_confirmation` (which would block
        // reading from a piped-stdin agent and fail closed).
        let ctx = ctx_requiring_confirmation(&mut tc);
        gate_mutation(&ctx, "jarvy_discover_apply", "write ./jarvy.toml").unwrap();
    }

    #[test]
    #[serial_test::serial]
    fn wizard_bypass_ignores_non_exact_env_values() {
        // Env var contract is EXACT match on "1". "0", "true", "YES",
        // empty, and whitespace variants must all fall through to the
        // normal confirmation path. This test only checks that the
        // bypass branch is NOT taken — the fall-through into
        // `prompt_mutation_confirmation` will error under a non-TTY
        // test environment, and either an error OR a panic is
        // acceptable evidence the bypass didn't fire. What is NOT
        // acceptable is `Ok(())` — that would mean the bypass matched.
        for bogus in ["0", "true", "TRUE", "YES", "yes", "", "1 ", " 1", "01"] {
            let dir = TempDir::new().unwrap();
            let mut tc = TestCtx::new(dir.path());
            let _guard = WizardEnvGuard::set(bogus);
            let ctx = ctx_requiring_confirmation(&mut tc);
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                gate_mutation(&ctx, "jarvy_discover_apply", "test")
            }));
            match result {
                Ok(Ok(())) => panic!(
                    "wizard bypass fired for non-exact env value `{bogus}` — \
                     JARVY_WIZARD_SESSION must be exactly `1`"
                ),
                Ok(Err(_)) => { /* expected: fell through, errored on TTY prompt */ }
                Err(_) => { /* expected: fell through, panicked reading from non-TTY stdin */ }
            }
        }
    }

    #[test]
    #[serial_test::serial]
    fn wizard_bypass_still_checks_rate_limit() {
        // The bypass is layered AFTER the rate-limit check in
        // `gate_mutation`. A saturated rate limiter must refuse the
        // call regardless of the wizard bypass — otherwise the bypass
        // becomes a rate-limit escape hatch.
        let dir = TempDir::new().unwrap();
        let mut tc = TestCtx::new(dir.path());
        // Zero-permit rate limiter so any call is denied.
        let mut config = McpConfig::default();
        config.mcp.require_confirmation = true;
        config.mcp.max_installs_per_minute = 0;
        tc.rate_limiter = RateLimiter::new(&config);
        tc.config = config;
        let _guard = WizardEnvGuard::set("1");
        let ctx = MutationCtx {
            config: &tc.config,
            rate_limiter: &tc.rate_limiter,
            audit_log: &tc.audit_log,
            client_name: Some("claude-code"),
            workspace_root: tc.workspace_root.clone(),
        };
        let result = gate_mutation(&ctx, "jarvy_discover_apply", "test");
        assert!(
            result.is_err(),
            "wizard bypass must NOT skip the rate-limit gate — \
             saturated limiter must refuse the call"
        );
    }

    #[test]
    #[serial_test::serial]
    fn wizard_bypass_writes_single_audit_entry_not_two() {
        // Regression guard for the fixed double-audit bug: the
        // pre-flight `log_mcp_mutation` at the top of gate_mutation
        // records the request. The wizard-bypass arm used to emit a
        // second, indistinguishable `log_mcp_mutation` — auditors
        // saw two rows per wizard-driven mutation, forensically
        // useless. The fix drops the redundant call. This test uses
        // an audit log wired to a temp file (not `AuditLog::disabled`)
        // and counts entries after one bypass call.
        let dir = TempDir::new().unwrap();
        let audit_dir = dir.path().join("audit");
        std::fs::create_dir_all(&audit_dir).unwrap();
        let mut config = McpConfig::default();
        config.mcp.require_confirmation = true;
        config.mcp.audit_log = audit_dir
            .join("audit.log")
            .to_string_lossy()
            .into_owned();
        let rate_limiter = RateLimiter::new(&config);
        let audit_log = AuditLog::new(&config).expect("audit log must init");
        let ctx = MutationCtx {
            config: &config,
            rate_limiter: &rate_limiter,
            audit_log: &audit_log,
            client_name: Some("claude-code"),
            workspace_root: dir.path().to_path_buf(),
        };
        let _guard = WizardEnvGuard::set("1");
        gate_mutation(&ctx, "jarvy_discover_apply", "write ./jarvy.toml").unwrap();
        drop(audit_log);

        let audit_content = std::fs::read_to_string(audit_dir.join("audit.log"))
            .unwrap_or_default();
        // Each entry is a single JSON line. Count `mcp_mutation` rows.
        let mutation_count = audit_content
            .lines()
            .filter(|l| l.contains("\"action\":\"mcp_mutation\""))
            .count();
        assert_eq!(
            mutation_count, 1,
            "wizard-bypass must produce exactly ONE mcp_mutation audit \
             entry (the pre-flight one). The old code double-logged. \
             Audit log contents:\n{audit_content}"
        );
    }

    #[test]
    #[allow(unsafe_code)]
    fn is_wizard_session_returns_false_when_env_absent() {
        // Under `#[cfg(test)]` the check re-reads the env var (no
        // LazyLock), so removing the var right before the check is
        // observable. Sanity-guard against future refactors that
        // accidentally re-add memoization to the test path.
        unsafe { std::env::remove_var("JARVY_WIZARD_SESSION") };
        assert!(!is_wizard_session(), "no env var → no bypass");
    }
}
