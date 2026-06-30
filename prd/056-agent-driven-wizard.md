# PRD-056 — Agent-driven setup wizard

## Status

Implemented (skill-drop + headless modes shipped together).

## Problem

Today's first-run path makes the user pick a template they don't
recognize, then run `jarvy discover --apply` to bootstrap a starter
`jarvy.toml`, then `jarvy setup` to install. Each step asks for
knowledge the user doesn't have yet. The DX gap is widest for
greenfield projects — exactly when "I just want this set up" is loudest.

PostHog/wizard solves the analogue for PostHog SDK integration by
handing the project to an AI agent. We want the same shape, but using
the user's own local AI subscription (Claude Code, Codex, Cursor,
Windsurf, Cline, Continue), not a vendor LLM gateway.

## Goals

- Single command `jarvy wizard` that hands the project to the user's
  AI agent for analysis + configuration.
- Works greenfield (no `jarvy.toml`) and against existing configs.
- Uses Jarvy's existing MCP server as the action surface — no new
  mutating tools, no API-key handling, no LLM SDK dependencies.
- Falls back to `jarvy quickstart` (the existing first-run flow) when
  no AI agent is installed, so users without an agent aren't blocked.

## Non-goals

- No PostHog-style LLM gateway. Jarvy is open-source and won't
  subsidize tokens.
- No direct Anthropic / OpenAI API client. Token auth stays on the
  user's agent CLI.
- No multi-turn conversation loop in headless mode (v1 is one-shot;
  users who want chat use skill-drop mode + their agent's UI).
- No IDE plugin integrations — Cursor / Windsurf are reached via
  skill drop.

## Design

### Two modes, picked per detected agent

**Mode A — Headless CLI** (Claude Code via `claude -p`, Codex via
`codex exec --`). Jarvy spawns the agent's CLI in non-interactive
mode and pipes a system prompt + project-context envelope to stdin.
The agent calls Jarvy's MCP tools inline. Streams the agent's
stdout/stderr straight to the user's terminal.

**Mode B — Skill drop** (Cursor, Windsurf, Cline, Continue, or any
agent when `--skill-only`). Jarvy writes a `jarvy-setup` `SKILL.md`
to the agent's skills dir, then prints a one-liner the user can paste
into the agent ("set up jarvy for this project") to activate it.

### Mode picker

1. `--agent <slug>` explicit override wins.
2. Else, headless CLI is preferred when the agent's CLI is on PATH
   (`claude` > `codex` per `Agent::ALL` ordering).
3. Else, skill-drop on the first detected GUI agent.
4. Else, fall back to `jarvy quickstart` — the existing first-run
   flow for users without any AI agent.

### Trust boundaries

- **Remote config refusal** — wizard refuses to `--apply` against a
  `jarvy.toml` tagged `ConfigOrigin::Remote`. Mirrors the
  `[packages] allow_remote` pattern.
- **Sandbox refusal** — `sandbox::is_sandbox() = true` → refuse.
  Wizard is for the host shell, not for in-agent sub-processes.
- **CI refusal** — `ci::is_ci() = true` → refuse. CI workflows that
  want this can set `JARVY_WIZARD=1`.
- **Non-TTY refusal (headless only)** — headless mode needs a TTY so
  the user can answer agent prompts. Skill-drop is TTY-agnostic.
- **Prompt content** — flows through `observability::sanitizer`
  before being passed to the agent, so secrets in source don't leak
  into stdout / shell history.

### MCP surface

One new MCP tool ships: `jarvy_wizard_plan` (read-only). Returns the
proposed setup plan as JSON — detections, required / recommended
tools, uninstallable bucket, plus a `greenfield` boolean. The agent
calls this first, presents the plan to the user, then invokes
`jarvy_discover_apply` (and optionally `jarvy_ai_hooks_apply`,
`jarvy_mcp_register_apply`) to commit changes. Every mutating tool
the wizard chains is gated by the existing `MutationCtx` (rate limit
+ TTY confirm + audit log).

### Telemetry

| Event | Fields |
|---|---|
| `wizard.started` | `mode`, `agent`, `apply`, `skill_only` |
| `wizard.skill_dropped` | `agent`, `skill_path` |
| `wizard.headless_spawned` | `agent`, `cmd_argv0` |
| `wizard.headless_exit` | `agent`, `exit_code`, `wall_ms` |
| `wizard.refused` | `reason ∈ {sandbox, ci, non_tty, remote_config, no_agent_installed, skill_drop_failed, headless_spawn_failed}` |

All gated on `observability::telemetry_gate::is_enabled()`. Added to
the canonical event taxonomy in CLAUDE.md.

## Open questions

- **Codex CLI flag stability.** OpenAI's CLI is younger than Claude
  Code's; if `codex exec --` semantics change, the wizard's spawn
  args break. Mitigation: integration test exercising the spawn flag
  set against the real CLI when present.
- **Claude Code skill loading semantics.** Verified by hand that
  `~/.claude/skills/<name>/SKILL.md` is picked up; if Claude's loader
  changes (e.g., requires a manifest index), the skill drop pipeline
  needs an extra step. Re-verify against each Claude Code release.
- **Per-agent prompt phrasing.** Currently all agents see the same
  prompt body. If one agent reasons systematically worse from the
  shared template, per-agent prompt variants land in
  `wizard::prompt`.

## Verification

See plan file (`/Users/zacclifton/.claude/plans/jazzy-nibbling-gem.md`)
verification section.
