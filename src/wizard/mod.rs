//! `jarvy wizard` — agent-driven setup (PRD-056).
//!
//! Hands the project to the user's local AI coding agent (Claude Code,
//! Codex, Cursor, Windsurf, Cline, Continue) so it can analyze the
//! repo and configure a `jarvy.toml` via Jarvy's existing MCP server.
//!
//! Two interaction modes:
//!
//! 1. **Headless CLI** (claude, codex) — `jarvy wizard` shells out to
//!    the agent's CLI with a system prompt + project-context envelope.
//!    The agent calls Jarvy's MCP tools inline (`jarvy_discover_apply`,
//!    `jarvy_mcp_register_apply`, etc.) and the user reviews the
//!    resulting plan summary.
//! 2. **Skill drop** (Cursor, Windsurf, Cline, Continue, or any agent
//!    when `--skill-only`) — writes a `jarvy-setup` `SKILL.md` to the
//!    agent's skills dir. The user opens their agent and types a
//!    one-liner ("set up jarvy for this project"); the skill drives
//!    the rest via Jarvy's already-registered MCP server.
//!
//! Mode picker (in order):
//! 1. `--agent <slug>` explicit override.
//! 2. Headless CLI: `claude` > `codex` if installed.
//! 3. Skill drop on the first detected GUI agent.
//! 4. **Fallback**: `jarvy quickstart` — preserves the existing
//!    first-run experience for users without any AI agent installed.
//!
//! Greenfield (no `jarvy.toml` at cwd) is the headline case — the
//! prompt explicitly instructs the agent to call `discover --apply`
//! first. No special-casing in Rust; the agent does the bootstrap
//! via the same MCP tool any user would.

pub mod context;
pub mod headless;
pub mod prompt;
pub mod session;
pub mod skill_drop;

use crate::agents::Agent;

/// Caller-facing options for `run_wizard`.
#[derive(Debug, Clone)]
pub struct WizardOpts {
    /// User-supplied `--agent <slug>` override (case-insensitive).
    pub agent_override: Option<String>,
    /// `--skill-only` — skip headless CLI even if available.
    pub skill_only: bool,
    /// `--apply` — commit changes. Default false (preview only).
    pub apply: bool,
    /// Output format: `"pretty"` (default) or `"json"`.
    pub output_format: String,
    /// `--file` — path to `jarvy.toml` (or where one would be written).
    pub config_file: String,
}

/// Selected interaction mode after the picker runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WizardMode {
    /// Spawn the agent's CLI directly with a system prompt.
    Headless { agent: Agent, cli_command: String },
    /// Install a skill into the agent's skills dir; print instructions.
    SkillDrop { agent: Agent },
    /// No agent installed — delegate to `jarvy quickstart`.
    QuickstartFallback,
}

/// Pick the interaction mode based on opts + which agents are
/// installed. Pure function — no I/O — so unit-testable.
///
/// `installed_agents` is computed by the caller via `Agent::is_installed()`
/// in the same pass; we accept it as a parameter to keep this function
/// hermetic and to let tests inject arbitrary install states.
///
/// `cli_on_path` maps each `Agent` to whether its CLI is on `$PATH`
/// (`claude` / `codex` for the CLI-capable agents; GUI agents always
/// `false`). Caller resolves via `which::which`.
pub fn pick_mode(
    opts: &WizardOpts,
    installed_agents: &[Agent],
    cli_on_path: impl Fn(Agent) -> bool,
) -> WizardMode {
    // 1. Explicit override wins.
    if let Some(slug) = opts.agent_override.as_deref()
        && let Some(agent) = Agent::from_slug(slug)
    {
        // Override implies the user knows what they're doing —
        // honor `--skill-only` if set, otherwise prefer headless
        // when the override agent has a CLI command + it's on PATH.
        if !opts.skill_only
            && let Some(cmd) = agent_cli_command(agent)
            && cli_on_path(agent)
        {
            return WizardMode::Headless {
                agent,
                cli_command: cmd.into(),
            };
        }
        return WizardMode::SkillDrop { agent };
    }
    // Unknown slug — fall through to auto-detection rather than
    // refusing. The caller emits a warning.

    // 2. Headless CLI preferred unless --skill-only.
    if !opts.skill_only {
        for &agent in installed_agents {
            if let Some(cmd) = agent_cli_command(agent)
                && cli_on_path(agent)
            {
                return WizardMode::Headless {
                    agent,
                    cli_command: cmd.into(),
                };
            }
        }
    }

    // 3. Skill drop on the first installed agent.
    if let Some(&agent) = installed_agents.first() {
        return WizardMode::SkillDrop { agent };
    }

    // 4. Fallback: hand off to quickstart.
    WizardMode::QuickstartFallback
}

/// Map an agent to its headless-mode CLI command, if any. Only
/// agents with a documented non-interactive mode are listed. GUI
/// agents (Cursor, Windsurf, Cline, Continue) return `None`.
///
/// The order in `pick_mode`'s headless loop is the order in
/// `Agent::ALL` — Claude Code wins ties because it's first in `ALL`.
pub fn agent_cli_command(agent: Agent) -> Option<&'static str> {
    match agent {
        Agent::ClaudeCode => Some("claude"),
        Agent::Codex => Some("codex"),
        // Cursor/Windsurf/Cline/Continue are GUI-first; their CLIs
        // (where present) don't expose a documented headless prompt
        // surface. Skill drop is the right interaction mode.
        Agent::Cursor | Agent::Windsurf | Agent::Cline | Agent::Continue => None,
    }
}

/// Detect which agents are installed by checking their config dirs.
/// Wraps `Agent::is_installed()` to produce the slice `pick_mode`
/// expects.
pub fn detect_installed_agents() -> Vec<Agent> {
    Agent::ALL
        .iter()
        .copied()
        .filter(|a| a.is_installed())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> WizardOpts {
        WizardOpts {
            agent_override: None,
            skill_only: false,
            apply: false,
            output_format: "pretty".into(),
            config_file: "./jarvy.toml".into(),
        }
    }

    #[test]
    fn pick_quickstart_when_no_agents_installed() {
        let mode = pick_mode(&opts(), &[], |_| false);
        assert_eq!(mode, WizardMode::QuickstartFallback);
    }

    #[test]
    fn pick_headless_when_claude_installed_and_on_path() {
        let mode = pick_mode(&opts(), &[Agent::ClaudeCode], |a| a == Agent::ClaudeCode);
        assert_eq!(
            mode,
            WizardMode::Headless {
                agent: Agent::ClaudeCode,
                cli_command: "claude".into(),
            }
        );
    }

    #[test]
    fn pick_skill_drop_when_claude_installed_but_cli_not_on_path() {
        // Edge case: ~/.claude/ exists (e.g., from a prior install)
        // but the `claude` CLI binary isn't on PATH. Fall back to
        // skill drop rather than failing to spawn.
        let mode = pick_mode(&opts(), &[Agent::ClaudeCode], |_| false);
        assert_eq!(
            mode,
            WizardMode::SkillDrop {
                agent: Agent::ClaudeCode
            }
        );
    }

    #[test]
    fn pick_skill_drop_when_only_gui_agent_installed() {
        let mode = pick_mode(&opts(), &[Agent::Cursor], |_| false);
        assert_eq!(
            mode,
            WizardMode::SkillDrop {
                agent: Agent::Cursor
            }
        );
    }

    #[test]
    fn pick_headless_prefers_claude_over_codex_when_both_present() {
        let mode = pick_mode(&opts(), &[Agent::ClaudeCode, Agent::Codex], |a| {
            matches!(a, Agent::ClaudeCode | Agent::Codex)
        });
        match mode {
            WizardMode::Headless { agent, .. } => assert_eq!(agent, Agent::ClaudeCode),
            other => panic!("expected headless claude, got {other:?}"),
        }
    }

    #[test]
    fn skill_only_forces_skill_drop_even_with_claude_on_path() {
        let mut o = opts();
        o.skill_only = true;
        let mode = pick_mode(&o, &[Agent::ClaudeCode], |_| true);
        assert_eq!(
            mode,
            WizardMode::SkillDrop {
                agent: Agent::ClaudeCode
            }
        );
    }

    #[test]
    fn agent_override_wins_over_first_installed() {
        let mut o = opts();
        o.agent_override = Some("cursor".into());
        // Claude is installed but user explicitly wants Cursor.
        let mode = pick_mode(&o, &[Agent::ClaudeCode, Agent::Cursor], |_| false);
        assert_eq!(
            mode,
            WizardMode::SkillDrop {
                agent: Agent::Cursor
            }
        );
    }

    #[test]
    fn unknown_override_falls_through_to_auto_detect() {
        let mut o = opts();
        o.agent_override = Some("notarealagent".into());
        let mode = pick_mode(&o, &[Agent::ClaudeCode], |a| a == Agent::ClaudeCode);
        // Auto-detect path picks claude headless.
        match mode {
            WizardMode::Headless { agent, .. } => assert_eq!(agent, Agent::ClaudeCode),
            other => panic!("expected headless claude on fallthrough, got {other:?}"),
        }
    }

    #[test]
    fn agent_cli_command_only_returns_for_headless_capable() {
        assert_eq!(agent_cli_command(Agent::ClaudeCode), Some("claude"));
        assert_eq!(agent_cli_command(Agent::Codex), Some("codex"));
        assert_eq!(agent_cli_command(Agent::Cursor), None);
        assert_eq!(agent_cli_command(Agent::Windsurf), None);
        assert_eq!(agent_cli_command(Agent::Cline), None);
        assert_eq!(agent_cli_command(Agent::Continue), None);
    }
}
