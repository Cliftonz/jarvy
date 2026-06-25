---
title: "AI Sandboxes - Jarvy"
description: "Use Jarvy to provision AI coding sandboxes — Claude Code, Cursor agents, Devin, OpenAI Codex, e2b, Modal, Daytona, devcontainers, Codespaces. Auto-detected seamless mode means no env-var rituals."
---

# Jarvy in AI Sandboxes

AI coding sandboxes are ephemeral execution environments where an agent
clones a repo, writes code, runs commands, and exits. Examples: Claude
Code sandbox, Cursor background agents, Devin, OpenAI Codex, Replit
Agent, e2b, Modal, Daytona, plus the long-running container family
(devcontainers, GitHub Codespaces, Gitpod).

The same properties that make Jarvy safe to wire into a project's
startup path — declarative, idempotent, fast no-op when satisfied,
exits clean in non-interactive contexts — make it a good fit for these
sandboxes. One `jarvy.toml` provisions the human's laptop, the agent's
sandbox, and CI from a single source of truth, so drift between "what
works for me" and "what the agent has" is impossible by construction.

## Seamless mode (it just works)

Jarvy auto-detects when it's running inside a sandbox or agent
environment and switches into **seamless mode**. You do not need to
set any environment variables to get the right behavior.

When seamless mode is active, Jarvy:

- **Disables interactive prompts.** No first-run setup wizard, no
  confirmation dialogs.
- **Auto-disables telemetry.** Despite the global opt-out default,
  multi-tenant base images must not leak signal across tenants —
  unattended sandboxes drop back to off. Per-session re-enable via
  `JARVY_TELEMETRY=1` still works for fleet operators who want
  session-scoped traces.
- **Disables auto-update checks.** Ephemeral envs shouldn't try to
  swap their own binary mid-session.
- **Auto-baselines drift state on first clean run.** If the sandbox
  image already has all the configured tools at the right versions
  and there's no existing `.jarvy/state.json`, Jarvy silently writes
  one. Future `jarvy drift check` runs in the same sandbox have a
  meaningful baseline without the operator running `jarvy drift
  accept` at image bake time.
- **Falls back to verify-only on read-only / no-install sandboxes.**
  If the rootfs is read-only or there's no sudo + no user-scope
  package manager available, Jarvy doesn't attempt installs. It
  runs the version-check pipeline and exits non-zero if anything
  required by `jarvy.toml` is missing — fast feedback instead of
  permission-denied spam.

You'll see one stderr line when seamless mode activates:

```
[jarvy] detected GitHub Codespaces — seamless mode (override: JARVY_SANDBOX=0)
```

If you ever need to disable it, set `JARVY_SANDBOX=0` for the
invocation.

### What gets auto-detected

| Sandbox / environment | Signal |
|---|---|
| GitHub Codespaces | `CODESPACES=true` or `CODESPACE_NAME` set |
| Gitpod | `GITPOD_WORKSPACE_ID` set |
| Devcontainers (VS Code Remote Containers) | `REMOTE_CONTAINERS=true` or `DEVCONTAINER=true` |
| Replit | `REPL_ID` set |
| e2b | `E2B_SANDBOX_ID` set |
| Modal | `MODAL_TASK_ID` set |
| Daytona | `DAYTONA_WS_ID` set |
| Claude Code | `CLAUDECODE=1` or `CLAUDE_CODE_ENTRYPOINT` set |
| Cursor background agents | `CURSOR_AGENT=1` |
| Generic container (fallback) | `/.dockerenv` exists AND stdin is not a TTY |
| Any CI runner | already covered by `crate::ci::is_ci()` — GitHub Actions, GitLab CI, CircleCI, etc. |

The generic-container fallback requires *both* `/.dockerenv` and a
non-TTY stdin so a developer who shelled into their own container
still gets normal interactive behavior.

## Two install strategies

| Strategy | When to pick |
|---|---|
| **Bake Jarvy into the base image** | Hot path. Image build runs the install script once. Session start runs `jarvy setup`, which is a fast no-op when satisfied. No network needed at session start, fastest cold boot. |
| **Install on session start** | Disposable templates with no custom base image. Entrypoint runs `curl … \| bash` then `jarvy setup`. Slower first boot, zero image maintenance. |

For agent sandboxes that spin up dozens of containers per hour, bake
into the image. For one-off ephemeral envs, install on start.

## Wire-up by sandbox

### Devcontainers / GitHub Codespaces / Gitpod

`devcontainer.json`:

```jsonc
{
  "name": "Project dev container",
  "image": "mcr.microsoft.com/devcontainers/base:ubuntu",
  "onCreateCommand": "curl -fsSL https://raw.githubusercontent.com/bearbinary/jarvy/main/dist/scripts/install.sh | bash",
  "postCreateCommand": "jarvy setup"
}
```

`onCreateCommand` runs during image build and is cached across
rebuilds. `postCreateCommand` runs per container, so it picks up any
config changes since the image was baked. No `JARVY_TEST_MODE`,
`CI=true`, or `JARVY_TELEMETRY` rituals required — Codespaces /
devcontainers are auto-detected.

For Codespaces specifically, see also the
[migration guide from Codespaces](migrate/from-codespaces.md).

### Dockerfile-based templates (e2b, Modal, Daytona, custom)

```dockerfile
FROM ubuntu:24.04

# Bake Jarvy into the image
RUN curl -fsSL https://raw.githubusercontent.com/bearbinary/jarvy/main/dist/scripts/install.sh | bash

# Project config — checked in alongside the Dockerfile
COPY jarvy.toml /workspace/jarvy.toml
WORKDIR /workspace

# Provision at image-build time so cold start is instant
RUN jarvy setup

# Drift-check at session start — auto-baseline writes state.json on the
# first run inside a detected sandbox, so this becomes a real check
# from the second session onward
ENTRYPOINT ["sh", "-c", "jarvy drift check || jarvy setup; exec \"$@\"", "--"]
```

Auto-baseline turns a pre-loaded image into a drift-trackable
environment without the operator running `jarvy drift accept` at image
bake time.

### Claude Code sandbox / Cursor background agents

For sandboxes that mount the project repo without a custom image, use
the graceful-skip pattern from
[for-ai-agents.md](for-ai-agents.md#integration-quickstart-integrate-jarvy-into-this-project).
Wire it into `package.json` / `Makefile` / `justfile`:

```jsonc
{
  "scripts": {
    "setup": "jarvy setup",
    "predev": "command -v jarvy >/dev/null 2>&1 && jarvy setup || echo 'jarvy not installed — skipping auto-provision'",
    "prebuild": "command -v jarvy >/dev/null 2>&1 && jarvy setup || echo 'jarvy not installed — skipping auto-provision'"
  }
}
```

Sandboxes without Jarvy still boot (one-line hint, no hard error).
Sandboxes with Jarvy auto-provision; Jarvy itself detects whether it's
running in Claude Code / Cursor and disables prompts/telemetry/updates
without you having to think about it.

### Claude Code: also register the MCP server

For Claude Code specifically, drop a `.mcp.json` in the repo so the
agent can install tools mid-task via the typed, audited
[MCP server](mcp-server.md) instead of shelling out:

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

Benefits over shell: dry-run is the default, tool calls are
rate-limited, every call is written to `~/.jarvy/mcp-audit.log` for
review.

## Override knobs

You should never need these in a normal sandbox. Documented here for
the cases where seamless mode guesses wrong:

| Variable | Effect |
|---|---|
| `JARVY_SANDBOX=0` | Force-disable sandbox detection for this invocation. Use when you're in a container you own and want full interactive setup. |
| `JARVY_SANDBOX=1` | Force seamless mode even when no sandbox signal is present. Useful for testing wire-up locally. |
| `JARVY_TELEMETRY=1` | Re-enable telemetry inside a seamless-mode session (default is off). Pair with `JARVY_OTLP_ENDPOINT` for tenant-scoped export. |
| `JARVY_UPDATE=1` | Re-enable update checks inside a seamless-mode session. |
| `JARVY_QUIET=1` | Suppress the seamless-mode banner. |

The legacy knobs (`JARVY_TEST_MODE=1`, `CI=true`) still work, but
they're redundant inside a detected sandbox. Set them only if you
need the matching semantics in an environment Jarvy doesn't recognize.

## Telemetry inside sandboxes

This is the part that bit operators before auto-detection landed. The
short version: telemetry is **off by default** in every detected
sandbox.

If you want session-scoped telemetry — say, you're running an agent
fleet and want OTLP traces of what each session installs — opt in
explicitly per-session:

```bash
export JARVY_TELEMETRY=1
export JARVY_OTLP_ENDPOINT="$TENANT_OTLP_ENDPOINT"
export OTEL_RESOURCE_ATTRIBUTES="tenant.id=$TENANT_ID,session.id=$SESSION_ID"
jarvy setup
```

Three guardrails:

1. **Don't bake `JARVY_OTLP_ENDPOINT` into a shared base image.** It
   leaks signal across tenants. Set it per-session.
2. **Don't enable telemetry in no-egress sandboxes.** If outbound HTTPS
   is blocked, every event becomes a 30-second connect-timeout. Pair
   `JARVY_TELEMETRY=1` with a sidecar OTLP collector on `localhost:4318`
   or leave it off.
3. **For K8s-hosted sandboxes**, the
   [`jarvy-telemetry-forwarder` Helm chart](https://github.com/bearbinary/jarvy/tree/main/dist/helm/jarvy-telemetry-forwarder)
   terminates OTLP in-cluster and forwards via whatever egress your
   cluster already allows. See
   [Telemetry forwarder operations](operations/telemetry-forwarder.md)
   for the deployment model.

## Drift detection as an AI safety net

The agent's job is to change code. Sometimes it also installs
unrelated things — a different Python version to test a hunch, a
debugger it forgets to uninstall, a global npm package that overrides
the project's pinned one. By the time the human reviews the PR, the
sandbox is gone and the install trail is gone with it.

Seamless mode turns the sandbox into an auditable surface
automatically. On the first session in a sandbox that already
matches `jarvy.toml`, Jarvy writes `.jarvy/state.json` as a baseline.
On subsequent sessions, `jarvy drift check` shows exactly what the
agent touched.

```bash
# Session end: dump what the agent touched
jarvy drift check --format json > /workspace/.jarvy-drift-report.json
```

Pipe the report into the agent's session summary, or fail the session
if drift exceeds a threshold. See [Drift detection](drift.md) for the
full state-file format and `accept` / `fix` workflows.

## Verify-only fallback

If Jarvy detects a read-only rootfs or a sudoless container without a
user-scope package manager on PATH, `jarvy setup` doesn't attempt
installs. It runs the version-check pipeline, prints a single stderr
line listing any gaps, and exits with code `3` (`PREREQ_MISSING`) if
anything is missing.

This is automatic — you don't need to call `jarvy doctor` instead of
`jarvy setup`. The same entrypoint works in install-capable and
read-only sandboxes; Jarvy figures out which one it's in.

To force this path during testing, set
`JARVY_FORCE_VERIFY_ONLY=1`.

## CI parity bonus

Because the same `jarvy.toml` provisions dev laptops and agent
sandboxes, you get CI parity for free:

```bash
jarvy ci-config github   # Emits a workflow that installs the same tools
```

Drop the emitted workflow into `.github/workflows/`. CI now installs
exactly what the agent's sandbox installed, which is exactly what the
human's laptop installed.

See [CI/CD integration](ci-cd.md) for per-provider snippets.

## Anti-patterns

- **Don't reach for `JARVY_TEST_MODE=1` / `CI=true` env vars in every
  sandbox entrypoint.** They're redundant in detected sandboxes.
  Setting them isn't wrong, just unnecessary.
- **Don't bake a telemetry endpoint into a shared sandbox base
  image.** It leaks signal across tenants. Set the endpoint
  per-session via `JARVY_OTLP_ENDPOINT` if you want session-scoped
  telemetry.
- **Don't skip `--dry-run` on first integration.** Run `jarvy setup
  --dry-run` once interactively, commit the resulting `jarvy.toml`,
  and only then point ephemeral sandboxes at it.
- **Don't bypass roles to make the agent's sandbox lighter.** Adding
  a second `jarvy.toml` ("the agent one") guarantees drift. Use
  `[roles.agent]` with `extends = "base"` instead.
- **Don't auto-accept drift in the agent's session.** Drift
  acceptance is a human decision. The auto-baseline on first run is
  intentionally *only* on a full clean match.

## Reference

- [For AI Agents overview](for-ai-agents.md) — three modes (use,
  configure, modify Jarvy)
- [MCP server](mcp-server.md) — typed, audited tool access for agents
- [Roles guide](roles.md) — per-role tool sets including agent-only
  roles
- [Drift detection](drift.md) — baseline + check + accept + fix
- [Network & proxy](network.md) — corporate egress, custom CA bundles
- [CI/CD integration](ci-cd.md) — emit matching workflows from the
  same config
- [Telemetry forwarder operations](operations/telemetry-forwarder.md)
  — in-cluster OTLP for multi-tenant sandbox fleets
- [Migrate from Codespaces](migrate/from-codespaces.md)
- [Migrate from Dev Containers](migrate/from-dev-containers.md)
- PRD-053 — implementation details and rationale for seamless mode
