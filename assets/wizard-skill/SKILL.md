---
name: jarvy-setup
description: Analyze the current project and produce a tailored jarvy.toml that installs the right developer tools, configures hooks, registers MCP servers, and sets up roles. Activates when the user asks to "set up jarvy", "configure jarvy for this project", or similar.
version: 1.0.0
---

# Jarvy setup skill

You are helping the user set up [Jarvy](https://jarvy.dev) — a
cross-platform CLI that provisions a developer environment from a
single declarative `jarvy.toml`. Your job is to analyze the project,
propose a config, and apply it through Jarvy's MCP server.

## Step 1 — Orient

Read the project structure first. Specifically check whether
`jarvy.toml` already exists at the project root:

- If yes: read it. Treat it as the user's baseline; you will refine
  rather than replace.
- If no: this is a **greenfield setup**. You will bootstrap a starter
  `jarvy.toml` in Step 3 before refining.

Also look at the surrounding tree:
- Marker files: `Cargo.toml`, `package.json`, `pyproject.toml`,
  `go.mod`, `Gemfile`, `Dockerfile`, `k8s/`, `*.tf`, `Makefile`,
  `Justfile`
- Version-pin files: `rust-toolchain.toml`, `.nvmrc`,
  `.python-version`, `go.mod`'s go directive
- CI: `.github/workflows/`, `.gitlab-ci.yml`, `.circleci/`

## Step 2 — Ask the MCP server what it sees

Call the `jarvy_wizard_plan` MCP tool. It runs `jarvy discover` against
the project and returns:
- Detected ecosystems (rust, node, python, …)
- Already-configured tools (entries already in `[provisioner]`)
- Required tools (high-confidence suggestions)
- Recommended companions (lower-confidence)
- Uninstallable detections (ecosystems jarvy can't install yet —
  surface these so the user knows to handle them manually)

Present the plan to the user before applying anything. Highlight any
divergence between what you'd suggest and what the tool detected. If
the user confirms, proceed to Step 3.

## Step 3 — Apply (idempotently)

**Idempotence is the hard rule.** Running this skill twice against
the same project MUST produce the same final `jarvy.toml`. The MCP
tools below are themselves idempotent (they merge, dedupe, and
refuse to overwrite); your job is to call them in the same order
with the same inputs every time.

**Check before you mutate.** Before calling any `*_apply` tool,
re-read the plan: if `required` is empty and `recommended` is
already filtered through user preference, there is nothing to do.
Say so and stop. Don't loop tool calls hoping for a different
result.

**Greenfield bootstrap (no `jarvy.toml` yet):**
- Call `jarvy_discover_apply` once with `apply = true`. This writes a
  starter `jarvy.toml` covering every detected ecosystem. The MCP
  tool refuses to overwrite an existing file — safe to call
  unconditionally on greenfield.

**Refinement (`jarvy.toml` already exists):**
- For each tool to add: call `jarvy_discover_apply` once with the
  filtered tool list — it merges into the existing `[provisioner]`
  table tool-by-tool. Repeated calls with the same input are
  no-ops; the tool emits `target = "noop"` telemetry in that case.
- For role-based configs: call `jarvy_roles_apply` if the user wants
  per-role tool sets (frontend / backend / data).
- For AI hooks (lint-on-save, redact-secrets, etc.): call
  `jarvy_ai_hooks_apply` with `dry_run = false`.

Each mutating MCP tool is rate-limited and emits an audit log entry.
Don't loop them silently — if a call returns `"status": "noop"` or
similar, that's the terminal state.

## Step 4 — Register the MCP server (if needed)

If `jarvy_mcp_register_check` shows the Jarvy MCP server isn't
registered for the user's other agents, offer to register it via
`jarvy_mcp_register_apply` so this skill works across all the user's
AI environments.

## Step 5 — Verify

Call `jarvy_validate_config` to confirm the resulting `jarvy.toml`
parses cleanly. If the user wants to run the install too, suggest:

```bash
jarvy setup
```

(Don't run `jarvy setup` for them — installing system packages should
be an explicit user-typed command, not a tool call.)

## Hard rules

- **Never modify files outside the project root.** Containment is
  enforced server-side, but you should still confine your reasoning
  to the current project tree.
- **Never write secrets into `jarvy.toml`.** If you find API keys or
  tokens in env files, stop and tell the user to use `[secrets]` or
  an external secret manager — don't echo them.
- **Don't suggest tools jarvy can't install.** The `uninstallable`
  bucket in `jarvy_wizard_plan`'s output names them; treat that list
  as advisory output only.
- **Don't run `jarvy setup` or `cargo install` for the user.** This
  skill configures the project, not the user's machine. Setup is a
  separate, explicit step.
- **Don't auto-apply without confirmation when the existing
  `jarvy.toml` was loaded from a remote source.** The MCP tool will
  refuse, but you should also recognize the
  `wizard.refused {reason: "remote_config"}` response and explain it.

## What success looks like

- `jarvy.toml` exists at the project root, parses cleanly, and
  declares every detected ecosystem.
- The user knows the next command is `jarvy setup`.
- The skill report names every tool that was added and every
  uninstallable detection that was surfaced.
