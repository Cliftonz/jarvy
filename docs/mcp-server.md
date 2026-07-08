---
title: "MCP Server - Jarvy"
description: "Use Jarvy as an MCP server to let AI agents like Claude, GPT, and Cursor discover and install development tools."
---

# Jarvy MCP Server

Jarvy exposes a Model Context Protocol (MCP) server that enables LLMs like Claude, GPT, and other AI assistants to safely discover, verify, and install development tools.

## Quick Start

```bash
# Start the MCP server
jarvy mcp

# With custom configuration
jarvy mcp --config ~/.jarvy/custom-mcp.toml
```

### Run via npm (no jarvy install required)

The `jarvy-mcp` npm package (maintained in-repo at `packages/npm/`)
downloads the platform-native jarvy binary on install
(SHA256-verified against the release `SHA256SUMS.txt`) and exposes the MCP
server as a bin:

```bash
npx -y jarvy-mcp        # starts the MCP server on stdio
```

MCP client config:

```json
{
  "mcpServers": {
    "jarvy": {
      "command": "npx",
      "args": ["-y", "jarvy-mcp"]
    }
  }
}
```

### Run via Docker

`ghcr.io/cliftonz/jarvy` (built from `dist/docker/Dockerfile`) defaults to
the MCP server entrypoint. Run with `-i` so stdio stays attached:

```bash
docker run -i --rm ghcr.io/cliftonz/jarvy:latest
```

MCP client config:

```json
{
  "mcpServers": {
    "jarvy": {
      "command": "docker",
      "args": ["run", "-i", "--rm", "ghcr.io/cliftonz/jarvy:latest"]
    }
  }
}
```

Notes:

- The image pins `JARVY_MCP_WORKSPACE=/workspace`; mount your project there
  (`-v "$PWD:/workspace"`) if you want project-aware tools
  (`jarvy_validate_config`, `jarvy_drift_status`, ...) to see it.
- Tool *installation* inside the container installs into the container, not
  your host — the Docker image is best for discovery / verification /
  read-only workflows. Use a native install for host provisioning.
- Any CLI subcommand overrides the default: `docker run --rm
  ghcr.io/cliftonz/jarvy:latest --version`.

## Integration with AI Clients

### Claude Desktop

Add to your Claude Desktop configuration file:

**macOS**: `~/.config/claude-desktop/claude-desktop.json`
**Windows**: `%APPDATA%\Claude\claude-desktop.json`

```json
{
  "mcpServers": {
    "jarvy": {
      "command": "jarvy",
      "args": ["mcp"]
    }
  }
}
```

### Cursor

Add to your Cursor settings (`.cursor/mcp.json`):

```json
{
  "mcpServers": {
    "jarvy": {
      "command": "jarvy",
      "args": ["mcp"]
    }
  }
}
```

### VS Code with Continue

Add to your Continue configuration:

```json
{
  "models": [...],
  "mcpServers": {
    "jarvy": {
      "command": "jarvy",
      "args": ["mcp"]
    }
  }
}
```

## Available MCP Tools

The MCP server exposes 5 tools for tool management:

### jarvy_list_tools

List all tools Jarvy can install with optional filtering.

**Parameters:**
- `category` (optional): Filter by category - `language`, `database`, `container`, `cli`, `editor`, `all`
- `platform` (optional): Filter by platform - `macos`, `linux`, `windows`, `current`
- `search` (optional): Search tools by name

**Example:**
```json
{
  "name": "jarvy_list_tools",
  "arguments": {
    "search": "docker",
    "platform": "current"
  }
}
```

### jarvy_get_tool

Get detailed information about a specific tool, including installation methods and dependencies.

**Parameters:**
- `name` (required): Tool name (e.g., `git`, `docker`, `node`)

**Example:**
```json
{
  "name": "jarvy_get_tool",
  "arguments": {
    "name": "kubectl"
  }
}
```

**Response:**
```json
{
  "name": "kubectl",
  "command": "kubectl",
  "macos": { "brew": "kubectl" },
  "linux": { "uniform": "kubectl" },
  "windows": { "winget": "Kubernetes.kubectl" },
  "dependencies": {
    "strict": [],
    "flexible": ["minikube", "kind", "k3d", "docker", "podman"]
  },
  "default_hook": {
    "description": "Enable kubectl shell completion and 'k' alias"
  }
}
```

**Dependency Types:**
- `strict`: ALL listed tools must be installed (e.g., lazydocker requires docker)
- `flexible`: AT LEAST ONE of the listed tools must be available (e.g., kubectl needs any K8s cluster provider)

### jarvy_check_tool

Check if a tool is installed and get its version.

**Parameters:**
- `name` (required): Tool name to check

**Example:**
```json
{
  "name": "jarvy_check_tool",
  "arguments": {
    "name": "git"
  }
}
```

**Response:**
```json
{
  "name": "git",
  "installed": true,
  "version": "2.43.0",
  "path": "/usr/bin/git"
}
```

### jarvy_check_multiple

Check installation status of multiple tools at once.

**Parameters:**
- `tools` (required): Array of tool names

**Example:**
```json
{
  "name": "jarvy_check_multiple",
  "arguments": {
    "tools": ["git", "docker", "node", "rust"]
  }
}
```

### jarvy_install_tool

Install a development tool (requires user confirmation).

**Parameters:**
- `name` (required): Tool name to install
- `version` (optional): Version hint (default: `latest`)
- `dry_run` (optional): Preview without executing (default: `true`)

**Example:**
```json
{
  "name": "jarvy_install_tool",
  "arguments": {
    "name": "ripgrep",
    "dry_run": true
  }
}
```

**Safety Note:** By default, `dry_run` is `true` to prevent accidental installations. Set `dry_run: false` to actually install, which will prompt for user confirmation.

## Extended tools (AI hooks, MCP registration, drift, roles, services, templates, validation)

Beyond the tool-installer family, the MCP server exposes Jarvy's broader subsystems so an AI agent can introspect and drive them directly. All extended tools have the `jarvy_` prefix.

### Read-only tools

These run without rate limiting or confirmation:

| Tool | Purpose |
|---|---|
| `jarvy_ai_hooks_list` | Show configured AI hooks in `jarvy.toml`, or pass `library: true` to dump the 16 curated built-in hooks (`block-rm-rf`, `audit-log`, etc.). |
| `jarvy_ai_hooks_check` | Diff configured AI hooks against each agent's settings file. Returns `missing` + `extra_jarvy` per agent. |
| `jarvy_mcp_register_list` | Show the configured MCP server registration block. Always reports the built-in `jarvy` server plus any allow-listed custom servers. |
| `jarvy_mcp_register_check` | Drift detection for MCP registrations across every targeted agent. |
| `jarvy_drift_check` / `jarvy_drift_status` | Surface the project's drift baseline state (`.jarvy/state.json`) — tools tracked, file count, config hash. |
| `jarvy_roles_list` / `jarvy_roles_show` | List roles defined in `jarvy.toml` and dump one role's inheritance + tool list. |
| `jarvy_services_status` | Detect whether the project has docker-compose or Tilt configured and whether the backend is installed. |
| `jarvy_templates_list` / `jarvy_templates_show` | Enumerate built-in templates (`node-bun`, `python-uv`, `k8s-platform`, ...) and show one template's full tool list + metadata. |
| `jarvy_validate_config` | Parse `jarvy.toml` and return whether it's valid plus a one-line summary (tool count, which subsystems are configured). Returns `error_type` (`missing` / `io` / `parse`) and `message` on failure. |

**Not yet exposed as first-class MCP tools** (reachable via the CLI subprocess with `--format json` — see PRD-051 for the universal JSON contract):

- `jarvy discover [--apply] [--missing] --format json` — auto-discover tools from project marker files (PRD-044). Shell out via the model's terminal tool; the JSON envelope is documented in `docs/discover.md`.
- `jarvy workspace {list,show,validate} --format json` — monorepo inspection (PRD-047). See `docs/workspace.md`.

These will likely get dedicated `jarvy_discover_*` / `jarvy_workspace_*` MCP tool wrappers in a follow-up — the CLI is the source of truth today.

### Mutating tools

These default to `dry_run: true` (preview only). Set `dry_run: false` and the call goes through the shared mutation guard: rate limit → stderr confirmation prompt → audit log entry → execute. The prompt fails closed in non-interactive mode (stderr not a TTY) and returns `-32005` (user declined) so a headless agent cannot drive a mutation without a human in the loop.

| Tool | Purpose |
|---|---|
| `jarvy_ai_hooks_apply` | Provision AI hooks to every configured agent. `dry_run: true` returns counts and would-refuse lists without writing. |
| `jarvy_mcp_register_apply` | Register MCP servers (Jarvy + allow-listed customs) with every targeted agent. |
| `jarvy_services_start` | Start the project's docker-compose / Tilt backend. `project_dir` is resolved through the workspace guard (see below) and refused if it escapes the workspace. `dry_run: true` reports which backend is detected and whether it's installed without invoking it. |
| `jarvy_templates_use` | Scaffold a `jarvy.toml` from a built-in template. `output_path` is resolved through the workspace guard, refusing absolute paths outside the workspace, `..` traversal, and symlink endpoints. On `dry_run: false`, the existing file (if any) is backed up to `<path>.bak` and the new content is written atomically (tempfile → fsync → rename). `force: true` overrides the no-overwrite default. |

### Workspace containment

Caller-supplied paths (`project_dir`, `output_path`) are resolved against an MCP workspace root and refused if they escape it. The workspace is:

- `JARVY_MCP_WORKSPACE` if set (absolute path the host pins explicitly), otherwise
- the server's current working directory at startup.

The guard canonicalizes the workspace, rejects absolute paths that don't sit under it, refuses any `..` component that would walk above it, and refuses to follow a symlink at the requested endpoint. This applies to both `jarvy_services_start` and `jarvy_templates_use`.

### Common parameters

All tools that read project state accept `config_path` (defaulting to `./jarvy.toml`) or `project_dir` (defaulting to cwd). Tools that fail closed when their inputs aren't present return a JSON envelope with `configured: false` or `baseline_exists: false` — never throw a JSON-RPC error for routine "not yet set up" cases.

### Example

```json
{
  "name": "jarvy_ai_hooks_list",
  "arguments": { "library": true }
}
```

Returns the curated set of 16 hooks (`block-rm-rf`, `block-secrets-commit`, `audit-log`, ...) so the agent can suggest which to enable for the user.

## Available MCP Resources

### jarvy://tools/index

Complete tool index as JSON with all supported tools.

### jarvy://platform/info

Current platform information including OS, architecture, and available package managers.

### jarvy://tools/{name}

Detailed information for a specific tool (e.g., `jarvy://tools/docker`).

## Available MCP Prompts

### setup_dev_environment

Guided prompt for setting up a development environment.

**Arguments:**
- `project_type`: One of `rust`, `node`, `python`, `go`, `java`, `ruby`, `devops`, `data-science`

### diagnose_missing_tools

Diagnostic prompt that checks common development tools and suggests installations.

## Configuration

Create `~/.jarvy/mcp-config.toml` to customize MCP server behavior:

```toml
[mcp]
# Require confirmation before installing (default: true)
require_confirmation = true

# Default to dry-run mode (default: true)
default_dry_run = true

[rate_limits]
# Maximum tool checks per minute (default: 10)
checks_per_minute = 10

# Maximum installations per minute (default: 3)
installs_per_minute = 3

[allowlist]
# Only allow these tools to be installed (optional)
# If set, only listed tools can be installed
tools = ["git", "node", "python", "rust", "docker"]

# [denylist]
# Block specific tools (takes precedence over allowlist)
# tools = ["dangerous-tool"]

[audit]
# Enable audit logging (default: true)
enabled = true

# Custom log path (default: ~/.jarvy/mcp-audit.log)
# log_path = "~/.jarvy/mcp-audit.log"
```

## Safety Features

### Dry Run by Default

All installation requests default to `dry_run: true`, showing what would happen without executing. This prevents LLMs from accidentally installing software.

### Rate Limiting

The server implements sliding window rate limiting:
- **10 checks per minute** for `check_tool` and `check_multiple`
- **3 installs per minute** for `install_tool`

### Allowlist/Denylist

Control which tools can be installed:
- **Allowlist**: Only specified tools can be installed
- **Denylist**: Specified tools are blocked (takes precedence)

### User Confirmation

When `require_confirmation` is enabled (default), actual installations prompt for user confirmation via stderr (not through MCP responses). Extended mutating tools (`jarvy_ai_hooks_apply`, `jarvy_mcp_register_apply`, `jarvy_services_start`, `jarvy_templates_use`) use the same prompt and fail closed when stderr is not a TTY.

### Workspace Containment

The server pins a workspace root at startup from `JARVY_MCP_WORKSPACE` (absolute path) or falls back to the cwd. Caller-supplied paths for `jarvy_services_start` (`project_dir`) and `jarvy_templates_use` (`output_path`) are resolved against this root and refused if they escape it (absolute outside, `..` traversal, or a symlink at the endpoint). `jarvy_templates_use` additionally backs up any pre-existing file to `<path>.bak` and writes atomically (tempfile → fsync → rename).

### Audit Logging

All MCP operations are logged to `~/.jarvy/mcp-audit.log` in JSON Lines format:

```json
{"timestamp":"2026-01-16T12:36:40Z","action":"install_tool","client":"claude-desktop","tool":"ripgrep","success":true,"version":"14.1.0"}
```

## Tool Dependencies

Jarvy tools can declare dependencies on other tools. The MCP server exposes this information to help LLMs make intelligent installation decisions.

### Dependency Types

**Strict Dependencies** (`depends_on`): ALL listed tools must be installed for the dependent tool to function.

Example: `lazydocker` has strict dependency on `docker` because it directly uses Docker APIs.

**Flexible Dependencies** (`depends_on_one_of`): AT LEAST ONE of the listed tools must be available.

Example: `kubectl` has flexible dependency on `["minikube", "kind", "k3d", "docker", "podman"]` because it can work with any Kubernetes cluster provider.

### Dependency Information in Responses

When you call `jarvy_get_tool`, the response includes dependency information:

```json
{
  "name": "minikube",
  "dependencies": {
    "strict": [],
    "flexible": ["docker", "podman"]
  }
}
```

### Installation Order Considerations

When installing multiple tools, Jarvy automatically orders them based on dependencies:

1. Tools without dependencies are installed first
2. Tools with strict dependencies wait for ALL dependencies
3. Tools with flexible dependencies wait for the FIRST matching option in the install list

**Example:** Installing `[kubectl, minikube, docker]`:
- Order: `docker` → `minikube` → `kubectl`
- Reason: minikube needs docker/podman (docker in list), kubectl needs a K8s provider (minikube in list)

### Common Dependency Patterns

| Tool | Strict Deps | Flexible Deps | Notes |
|------|-------------|---------------|-------|
| lazydocker | docker | - | Docker TUI, needs daemon |
| kind | docker | - | Kubernetes-in-Docker |
| kubectl | - | minikube, kind, docker, ... | Any K8s cluster |
| helm | - | kubectl | Package manager for K8s |
| k9s | - | kubectl | K8s TUI |
| minikube | - | docker, podman | Local K8s cluster |
| dive | - | docker, podman | Image layer explorer |

### Best Practices for LLMs

1. **Check dependencies first:** Before installing a tool, check its dependencies via `jarvy_get_tool`
2. **Install dependencies together:** If a user wants kubectl, suggest also installing a cluster provider
3. **Respect user choice:** For flexible deps, ask which option the user prefers
4. **Warn about missing deps:** If strict deps are missing, inform the user the tool may not work

## Error Codes

| Code | Meaning |
|------|---------|
| -32700 | Parse error |
| -32600 | Invalid request |
| -32601 | Method not found |
| -32602 | Invalid params |
| -32603 | Internal error |
| -32001 | Tool execution failed |
| -32002 | Tool not found |
| -32003 | Installation denied (denylist) |
| -32004 | Rate limited |
| -32005 | User declined |
| -32006 | Sudo required |
| -32007 | Timeout |
| -32008 | Missing dependency |

## Troubleshooting

### Server not starting

1. Ensure Jarvy is installed and in your PATH:
   ```bash
   which jarvy
   jarvy --version
   ```

2. Test the MCP server manually:
   ```bash
   echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' | jarvy mcp
   ```

### Rate limit errors

If you see rate limit errors, wait 60 seconds for the sliding window to reset, or adjust limits in `~/.jarvy/mcp-config.toml`.

### Tools not installing

1. Check if the tool is in the denylist
2. Ensure `dry_run` is set to `false`
3. Verify user confirmation was accepted
4. Check audit log for error details: `tail -f ~/.jarvy/mcp-audit.log`

### Permission errors

Some tools require elevated permissions. The MCP server will return error code `-32006` (Sudo required) when this occurs.

## Distribution & MCP Registry

Jarvy's MCP server ships through three channels:

| Channel | Artifact | Source |
|---|---|---|
| Native binary | `jarvy mcp` (crates.io / brew / AUR / winget / installer script) | `.github/workflows/publish-packages.yml` |
| npm | `jarvy-mcp` (binary-download wrapper) | `packages/npm/` |
| Docker | `ghcr.io/cliftonz/jarvy` | `dist/docker/Dockerfile` + `.github/workflows/docker-publish.yml` (release tags / manual dispatch only) |

### Official MCP registry

`dist/mcp-registry/server.json` describes this server for the
[official MCP registry](https://registry.modelcontextprotocol.io/) under the
name `io.github.cliftonz/jarvy`, listing the npm and OCI packages above.

Submission is a **maintainer-only** action and requires the npm package and
Docker image to already be published (the registry validates package
ownership: `mcpName` in the npm `package.json`, and the
`io.modelcontextprotocol.server.name` label on the OCI image). The
step-by-step publish procedure (`mcp-publisher login github` +
`mcp-publisher publish`) is documented in `dist/mcp-registry/README.md`.

## Protocol Version

This implementation targets MCP protocol version `2024-11-05`.
