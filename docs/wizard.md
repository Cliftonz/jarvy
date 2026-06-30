# `jarvy wizard` — agent-driven setup

`jarvy wizard` hands your project to the AI coding agent you already
use (Claude Code, Codex, Cursor, Windsurf, Cline, or Continue) and
lets it set up Jarvy for you.

It does **not** call any vendor LLM directly. It uses *your* local
subscription via the agent's CLI (for headless agents) or by dropping
a skill into the agent's skill dir (for GUI agents). Jarvy pays no
tokens; you keep control of which subscription does the work.

## When to use it

- **Greenfield**: you just cloned a repo and there's no `jarvy.toml`
  yet. The wizard scans the tree, proposes a config, and writes it.
- **Refinement**: a `jarvy.toml` exists but you want to add tools
  for a new ecosystem (e.g., you just added a Rust crate to a
  TypeScript monorepo). The wizard suggests additions; existing
  entries are preserved.

## Quick start

```bash
# Preview only (default — wizard inspects but doesn't change anything)
jarvy wizard

# Apply changes
jarvy wizard --apply

# Pick a specific agent
jarvy wizard --agent claude-code --apply

# Always drop a skill (don't shell out, even if you have claude CLI)
jarvy wizard --skill-only --agent cursor
```

## How mode picking works

| Detected | Action |
|---|---|
| `claude` CLI on PATH | Spawn `claude -p` with the prompt on stdin. |
| `codex` CLI on PATH | Spawn `codex exec --` with the prompt on stdin. |
| Cursor / Windsurf / Cline / Continue installed | Write `SKILL.md` to that agent's skills dir; print instructions. |
| Nothing installed | Fall back to `jarvy quickstart` — the existing first-run flow. |

Pass `--agent <slug>` to force a specific agent. Pass `--skill-only`
to always use skill-drop mode regardless of detection.

## What gets installed (skill drop)

| Agent | Path |
|---|---|
| Claude Code | `~/.claude/skills/jarvy-setup/SKILL.md` |
| Cursor | `~/.cursor/skills/jarvy-setup/SKILL.md` |
| Codex | `~/.codex/skills/jarvy-setup/SKILL.md` |
| Windsurf | `~/.windsurf/skills/jarvy-setup/SKILL.md` |
| Cline | `~/.cline/skills/jarvy-setup/SKILL.md` |
| Continue | `~/.continue/skills/jarvy-setup/SKILL.md` |

Once installed, open your agent and type:

> "set up jarvy for this project"

Your agent will call Jarvy's MCP server (already registered via
`jarvy mcp-register`) to scan the tree, propose a plan, and apply
the config.

## What runs (headless)

`jarvy wizard --apply` with Claude Code on PATH:

1. Builds a project-context envelope (`jarvy discover` JSON +
   filtered tree listing).
2. Spawns `claude -p` with a system prompt that explains Jarvy's MCP
   tools.
3. Pipes the envelope to stdin and streams the agent's stdout to
   your terminal.
4. The agent reads the prompt, decides what to add, calls
   `jarvy_wizard_plan` for a read-only proposal, then
   `jarvy_discover_apply` to commit.
5. You see the conversation live.

## Trust boundaries

The wizard refuses to auto-apply in:

- **Sandbox** environments (e.g., when run from inside another AI
  agent's sandbox).
- **CI** environments (CI workflows shouldn't be interactively
  refining configs).
- **Non-TTY** stdin when running headless (it needs a TTY for agent
  prompts).
- **Remote configs** (a `jarvy.toml` pulled from a URL can't be
  auto-applied; preview only).

Override: `JARVY_WIZARD=1` forces the wizard to run anyway. Use it
when you understand the trade-off (e.g., a CI workflow that
specifically wants to bootstrap a `jarvy.toml` via an agent).

## The MCP tool surface

When your agent runs the skill or the headless prompt, it calls
these MCP tools (already exposed by Jarvy):

| Tool | Purpose |
|---|---|
| `jarvy_wizard_plan` | Read-only: detections, required + recommended tools, greenfield flag. Agent presents this before mutating anything. |
| `jarvy_discover_apply` | Mutating: merge / bootstrap `[provisioner]`. Rate-limited + audit-logged. |
| `jarvy_validate_config` | Read-only: confirm the resulting `jarvy.toml` parses. |
| `jarvy_ai_hooks_apply` | Optional: provision AI hooks across other agents. |
| `jarvy_mcp_register_apply` | Optional: register the Jarvy MCP server with other agents. |

The wizard does **not** add any new mutating tool. Every change goes
through `MutationCtx` (rate limit + TTY confirm + audit log).

## Falling back to `jarvy quickstart`

If you don't have any AI agent installed, `jarvy wizard` delegates to
`jarvy quickstart` — the same first-run experience users without an
agent get. You won't be blocked.

## See also

- [Quickstart](./quickstart.md) — manual first-run flow.
- [Discover](./discover.md) — the analyzer the wizard wraps.
- [MCP registration](./mcp-registration.md) — how the wizard's MCP
  tool surface is exposed.
- [Skills guide](./skills.md) — how skills are installed across agents.
