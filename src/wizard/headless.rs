//! Headless-mode: spawn the agent's CLI in non-interactive mode and
//! stream its output to the user's terminal.
//!
//! Two supported CLIs today — `claude` (Claude Code) and `codex`
//! (OpenAI Codex CLI). Each has a slightly different non-interactive
//! flag surface; the agent-specific differences are isolated to a
//! single `match agent` block in `spawn_args`.
//!
//! This module owns the process invocation only. Prompt building
//! lives in `super::prompt`; project envelope assembly in
//! `super::context`. Telemetry events are emitted by the caller
//! (`commands::wizard_cmd`) so we don't double-fire from here.

use crate::agents::Agent;
use std::io;
use std::process::{Command, ExitStatus, Stdio};

/// Environment variables the wizard exports to the spawned agent CLI.
/// Exposed publicly so tests + `commands::wizard_cmd` can reference the
/// exact names without repeating string literals (drift risk on refactor).
pub const WIZARD_SESSION_ENV: &str = "JARVY_WIZARD_SESSION";
pub const WIZARD_SESSION_ID_ENV: &str = "JARVY_WIZARD_SESSION_ID";

/// Generate a per-invocation session ID (UUID v7 — time-ordered for
/// log-stream correlation) exported via `WIZARD_SESSION_ID_ENV`. The
/// same UUID is threaded through telemetry so on-call can correlate
/// `wizard.headless_spawned` → `mcp.mutation.wizard_bypass` →
/// `discover.applied` → `wizard.headless_exit` across a single run.
pub fn new_session_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

#[derive(Debug, thiserror::Error)]
pub enum HeadlessError {
    /// `claude` / `codex` not on PATH after we said it was. Shouldn't
    /// happen if the caller validated via `which::which` first; kept
    /// for defensive error surfaces.
    #[error("`{0}` CLI not found on PATH")]
    CliMissing(String),

    /// The agent variant doesn't have a headless CLI mapping.
    /// (Cursor / Windsurf / Cline / Continue — these always skill-drop.)
    #[error("agent `{0}` has no headless CLI mode")]
    NotHeadlessCapable(String),

    /// `spawn()` itself failed (e.g., fork/exec error).
    #[error("spawn `{cmd}`: {source}")]
    Spawn {
        cmd: String,
        #[source]
        source: io::Error,
    },

    /// Process spawned but failed to wait. Rare on Unix.
    #[error("wait `{cmd}`: {source}")]
    Wait {
        cmd: String,
        #[source]
        source: io::Error,
    },
}

/// Build the argv for a given agent's headless invocation.
///
/// Note: the *prompt body* is piped via stdin rather than passed as
/// `--prompt "<body>"` for two reasons:
/// 1. Argv length limits on Windows (~32K) bite for large envelopes.
/// 2. Shell interpolation surprises (backticks, `$` expansion) can
///    corrupt the prompt; stdin bypasses the shell entirely.
///
/// Both CLIs read stdin when no positional prompt is supplied —
/// verified against `claude --help` (`claude -p` accepts stdin) and
/// `codex --help` (`codex exec --` reads stdin).
pub fn spawn_args(agent: Agent) -> Result<(&'static str, Vec<&'static str>), HeadlessError> {
    match agent {
        // Claude Code: `-p` is non-interactive ("print mode"). Without
        // a positional argument, the prompt is read from stdin.
        //
        // `--allowedTools "mcp__jarvy"` pre-approves every tool exposed
        // by the Jarvy MCP server (`jarvy_wizard_plan`,
        // `jarvy_discover_apply`, `jarvy_ai_hooks_apply`,
        // `jarvy_mcp_register_apply`, `jarvy_validate_config`, …).
        // Without this, `-p` blocks on the first MCP call waiting for
        // an interactive approval that never arrives, so the wizard
        // appears to hang. The allowlist is scoped to the Jarvy server
        // only — file edits, Bash, and other non-Jarvy tools still
        // surface the usual prompts.
        Agent::ClaudeCode => Ok(("claude", vec!["-p", "--allowedTools", "mcp__jarvy"])),
        // Codex: `exec` is the one-shot subcommand. `--` separates
        // flags from the prompt; with nothing after `--`, the prompt
        // is read from stdin.
        Agent::Codex => Ok(("codex", vec!["exec", "--"])),
        a => Err(HeadlessError::NotHeadlessCapable(a.slug().to_string())),
    }
}

/// Build a fully-configured `Command` for the agent's headless spawn.
///
/// Hoisted out of `run()` so tests can inspect argv and env without
/// actually forking. Test-visible seam: `get_envs()` on the returned
/// `Command` proves `JARVY_WIZARD_SESSION` / `JARVY_WIZARD_SESSION_ID`
/// were set — otherwise the whole bug this commit fixes could regress
/// silently on a merge conflict resolution that drops the `.env()`
/// lines.
///
/// `session_id` is a per-invocation UUID exported to the child so
/// downstream telemetry (`mcp.mutation.wizard_bypass`,
/// `discover.applied` fired from inside the wizard) can correlate to
/// the enclosing wizard run — otherwise concurrent wizard invocations
/// (dev + CI) produce indistinguishable events.
pub fn build_command(agent: Agent, session_id: &str) -> Result<Command, HeadlessError> {
    let (cmd, args) = spawn_args(agent)?;
    let mut command = Command::new(cmd);
    command
        .args(&args)
        // `JARVY_WIZARD_SESSION=1` is inherited by the agent CLI and,
        // in turn, by any `jarvy mcp` server it spawns via its MCP-
        // server config. The MCP mutation gate
        // (`mcp::extended_tools::gate_mutation`) treats this as
        // operator-pre-approved consent and skips the TTY confirmation
        // prompt — which would otherwise fail closed because stdin on
        // the agent is piped (used for the prompt body), leaving the
        // gate's `read_line` no way to read a "yes".
        .env(WIZARD_SESSION_ENV, "1")
        // Per-invocation UUID — used only for telemetry correlation.
        // Empty when the caller hasn't generated one yet (backwards
        // compat with older callers; new callers always populate).
        .env(WIZARD_SESSION_ID_ENV, session_id)
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    Ok(command)
}

/// Run the agent against the supplied prompt. Streams stdout/stderr
/// straight to the user's terminal (inherited stdio) so they see
/// the conversation live. Blocks until the agent exits.
///
/// Returns the exit status — the caller maps non-zero to
/// `error_codes::HOOK_FAILED` (or similar) and emits telemetry.
pub fn run(agent: Agent, prompt: &str) -> Result<ExitStatus, HeadlessError> {
    let session_id = new_session_id();
    let (cmd, _args) = spawn_args(agent)?;
    let mut child = build_command(agent, &session_id)?
        .spawn()
        .map_err(|e| match e.kind() {
            io::ErrorKind::NotFound => HeadlessError::CliMissing(cmd.to_string()),
            _ => HeadlessError::Spawn {
                cmd: cmd.to_string(),
                source: e,
            },
        })?;

    // Pipe the prompt body to the child's stdin and close — many CLI
    // agents wait for EOF before processing. Dropping the handle
    // closes the pipe cleanly.
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        stdin
            .write_all(prompt.as_bytes())
            .map_err(|e| HeadlessError::Spawn {
                cmd: cmd.to_string(),
                source: e,
            })?;
        // Explicit drop is documentation — `stdin` would close on
        // scope exit anyway; named drop pins the contract.
        drop(stdin);
    }

    child.wait().map_err(|e| HeadlessError::Wait {
        cmd: cmd.to_string(),
        source: e,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_args_for_claude_uses_p_flag() {
        let (cmd, args) = spawn_args(Agent::ClaudeCode).unwrap();
        assert_eq!(cmd, "claude");
        assert!(
            args.contains(&"-p"),
            "claude must run in print/non-interactive mode"
        );
    }

    #[test]
    fn spawn_args_for_claude_preapproves_jarvy_mcp() {
        // `-p` mode blocks on MCP permission prompts; the wizard's
        // playbook fires `jarvy_wizard_plan` / `jarvy_discover_apply`
        // / etc., so the Jarvy MCP server must be pre-allowlisted or
        // the spawned agent appears to hang. Scoped to `mcp__jarvy`
        // only — non-Jarvy tools still surface prompts.
        //
        // Full-argv equality (not just `contains`) — Claude Code's CLI
        // requires `-p` in a specific position for stdin-mode
        // detection in some releases; reordering the args silently
        // breaks the wizard. Pin the exact order so a future edit
        // that "cleans up" the argv order trips a compile-time-loud
        // test failure instead of an on-call-loud production failure.
        let (cmd, args) = spawn_args(Agent::ClaudeCode).unwrap();
        assert_eq!(cmd, "claude");
        assert_eq!(
            args,
            vec!["-p", "--allowedTools", "mcp__jarvy"],
            "argv order for claude must be [-p, --allowedTools, mcp__jarvy] \
             — Claude Code's flag parser is order-sensitive"
        );
    }

    /// The `--allowedTools mcp__jarvy` value is passed verbatim to
    /// Claude Code, which interprets `mcp__<server>` as a server-name
    /// prefix allowlist. If a future contributor "cleans up" the
    /// value to a bare `jarvy` (or an unrelated pattern), the scope
    /// silently broadens (any `mcp__*` server matches) or narrows to
    /// nothing (wizard hangs). Pin the exact prefix + the double-
    /// underscore delimiter so a drift-inducing edit trips a
    /// compile-time-loud test failure.
    ///
    /// Threat model call-out: even if a third-party tool were to
    /// register itself as `jarvy-experimental` MCP server (starting
    /// with `jarvy` but distinct), Claude Code's `mcp__jarvy` scope
    /// matches on the exact server name after the `mcp__` prefix, not
    /// a substring — verified against Claude Code docs 2026-07.
    /// Future release-note review needed if that behavior changes.
    #[test]
    fn allowed_tools_scope_pins_exact_jarvy_server_prefix() {
        let (_, args) = spawn_args(Agent::ClaudeCode).unwrap();
        let value = args
            .iter()
            .position(|a| *a == "--allowedTools")
            .and_then(|i| args.get(i + 1))
            .expect("--allowedTools has a value");
        assert!(
            value.starts_with("mcp__"),
            "allowedTools must use the `mcp__<server>` grammar; got: {value}"
        );
        assert!(
            value.contains("__jarvy"),
            "scope must reference the `jarvy` server explicitly, not \
             a naked prefix that could match `jarvy-experimental` or \
             `jarvy-labs` — got: {value}"
        );
        assert_eq!(
            *value, "mcp__jarvy",
            "scope must be EXACTLY `mcp__jarvy` — no whitespace, no \
             trailing wildcard, no other suffix. Any deviation risks \
             silent scope broadening in future Claude Code releases."
        );
    }

    #[test]
    fn build_command_for_claude_sets_wizard_session_env() {
        // Regression guard for the exact bug the wizard-runtime fix
        // ships: without JARVY_WIZARD_SESSION=1 propagating to the
        // spawned agent, its descendant `jarvy mcp` server falls
        // back to the TTY prompt (which fails closed because stdin
        // carries the prompt body). A merge-conflict resolution that
        // deletes the `.env()` line compiles + ships + silently
        // reverts the fix. This test proves the env is set.
        let session_id = "test-session-uuid";
        let cmd = build_command(Agent::ClaudeCode, session_id).unwrap();
        let envs: std::collections::HashMap<_, _> = cmd
            .get_envs()
            .filter_map(|(k, v)| v.map(|vv| (k.to_owned(), vv.to_owned())))
            .collect();
        assert_eq!(
            envs.get(std::ffi::OsStr::new(WIZARD_SESSION_ENV)),
            Some(&std::ffi::OsString::from("1")),
            "JARVY_WIZARD_SESSION=1 MUST be set on the spawn — this \
             is what marks descendant MCP-server processes as \
             wizard-driven and bypasses the mutation-confirmation TTY \
             prompt. Reverting the .env() line silently reverts the \
             fix; this test refuses to let that happen."
        );
        assert_eq!(
            envs.get(std::ffi::OsStr::new(WIZARD_SESSION_ID_ENV)),
            Some(&std::ffi::OsString::from(session_id)),
            "JARVY_WIZARD_SESSION_ID must be threaded so telemetry \
             events emitted from descendants can correlate to the \
             enclosing wizard invocation"
        );
    }

    #[test]
    fn build_command_for_codex_sets_wizard_session_env() {
        let cmd = build_command(Agent::Codex, "codex-session").unwrap();
        let envs: std::collections::HashMap<_, _> = cmd
            .get_envs()
            .filter_map(|(k, v)| v.map(|vv| (k.to_owned(), vv.to_owned())))
            .collect();
        assert_eq!(
            envs.get(std::ffi::OsStr::new(WIZARD_SESSION_ENV)),
            Some(&std::ffi::OsString::from("1"))
        );
    }

    #[test]
    fn new_session_id_is_unique_across_calls() {
        // UUID v7 is time-ordered but must be per-invocation unique so
        // concurrent wizard runs (dev + CI) produce distinct
        // correlation IDs. Two same-millisecond calls: both non-empty,
        // different, valid UUID shape.
        let a = new_session_id();
        let b = new_session_id();
        assert_ne!(a, b, "session ids must be per-invocation unique");
        assert_eq!(a.len(), 36, "UUID stringified length is 36");
    }

    #[test]
    fn spawn_args_for_codex_uses_exec_subcommand() {
        let (cmd, args) = spawn_args(Agent::Codex).unwrap();
        assert_eq!(cmd, "codex");
        assert!(args.contains(&"exec"));
    }

    #[test]
    fn spawn_args_rejects_gui_agents() {
        for &agent in &[
            Agent::Cursor,
            Agent::Windsurf,
            Agent::Cline,
            Agent::Continue,
        ] {
            assert!(
                spawn_args(agent).is_err(),
                "GUI agent `{}` must not have a headless mapping",
                agent.slug()
            );
        }
    }
}
