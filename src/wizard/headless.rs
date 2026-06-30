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
        Agent::ClaudeCode => Ok(("claude", vec!["-p"])),
        // Codex: `exec` is the one-shot subcommand. `--` separates
        // flags from the prompt; with nothing after `--`, the prompt
        // is read from stdin.
        Agent::Codex => Ok(("codex", vec!["exec", "--"])),
        a => Err(HeadlessError::NotHeadlessCapable(a.slug().to_string())),
    }
}

/// Run the agent against the supplied prompt. Streams stdout/stderr
/// straight to the user's terminal (inherited stdio) so they see
/// the conversation live. Blocks until the agent exits.
///
/// Returns the exit status — the caller maps non-zero to
/// `error_codes::HOOK_FAILED` (or similar) and emits telemetry.
pub fn run(agent: Agent, prompt: &str) -> Result<ExitStatus, HeadlessError> {
    let (cmd, args) = spawn_args(agent)?;

    let mut child = Command::new(cmd)
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
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
