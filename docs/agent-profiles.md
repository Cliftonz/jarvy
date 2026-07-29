---
title: "Agent Profiles - Jarvy"
description: "Snapshot and switch whole AI-agent configurations (work vs personal, per-client MCP allowlists) with jarvy agents profile — env-var switching per terminal, symlink switching globally."
---

# Agent Profiles

Developers accumulate multiple identities per AI agent: a work Claude Code
login with the client-approved MCP allowlist, a personal one with the
kitchen-sink toolset, a demo setup with nothing sensitive in it. Each
agent stores all of that — credentials, settings, skills, MCP
registrations — in one config directory, so "switching identity" today
means shuffling directories by hand.

`jarvy agents profile` makes those identities first-class: a **profile**
is a named snapshot of each agent's config directory, stored under
`~/.jarvy/agent-profiles/<profile>/<agent>/`, switchable per terminal or
globally.

```console
$ jarvy agents profile init          # snapshot current configs as 'default'
$ jarvy agents profile create work --from default
$ eval "$(jarvy agents profile use work --print-env)"   # this terminal only
$ jarvy agents profile use work --global                # everywhere
```

## Two switching tiers

Agents differ in whether their config-dir location can be redirected:

| Tier | Agents (v1) | Mechanism | Scope |
|------|-------------|-----------|-------|
| Env | claude-code (`CLAUDE_CONFIG_DIR`), codex (`CODEX_HOME`) | export an env var pointing into the profile store | **Per terminal** — two shells can run different profiles at once |
| Symlink | cursor | `~/.cursor` becomes a symlink into the profile store, atomically re-pointed | **Global** — all terminals and the IDE see the switch |

windsurf, cline, and continue are snapshotted into profiles (storage
works for all six agents) but not yet switchable — v1.1 territory, along
with git-based profile sync and IDE globalStorage handling.

## Commands

```
jarvy agents profile init   [--agents a,b]    # snapshot installed agents as 'default'
jarvy agents profile create <name> [--from <src>] [--agents a,b]
jarvy agents profile use <name> [--agents a,b] [--global] [--print-env]
jarvy agents profile list   [--format json|pretty]
jarvy agents profile status [--format json|pretty]
jarvy agents profile delete <name>
```

### `init`

Snapshots every installed agent into a profile named `default` and sets
it as the default. Env-tier agents are **copied** (the live directory
keeps working untouched); symlink-tier agents are **moved** into the
store with a symlink left at the original path. Idempotent — re-running
skips agents that are already managed.

`--agents claude-code,codex` narrows the snapshot. Use it when a
symlink-tier editor is open: that tier *moves* the live config directory,
which a running IDE will not appreciate.

## What a profile does and does not contain

A profile carries **identity**: credentials, `settings.json` /
`config.toml`, `CLAUDE.md` / `AGENTS.md`, skills, MCP registrations,
plugin selections.

It deliberately skips what an agent directory merely accumulates —
conversation transcripts (`~/.claude/projects`), re-downloadable package,
extension and marketplace trees (`~/.codex/packages`,
`~/.cursor/extensions`, `~/.claude/plugins/cache`), log databases and
scratch space. On one real machine that is the difference between 2.3 GB
and 2.9 MB per profile, and it stops `create --from` cloning one
identity's conversation history into another.

The rule set (`src/agent_profiles/exclude.rs`) is a denylist: a file
jarvy does not recognize is **kept**. A profile being larger than
necessary is a nuisance; a profile silently missing config you rely on is
a bug. Live IPC endpoints such as `~/.codex/ipc/ipc.sock` are skipped by
file type — they are not copyable, and one of them used to fail the whole
snapshot.

### `use`

Without `--global`, `use` affects env-tier agents only and prints the
`export` lines for your shell (symlink-tier agents get a stderr notice
that their switch is global). `--print-env` makes stdout carry *only*
the export lines — human text goes to stderr — so the output is safe to
`eval`:

```console
$ eval "$(jarvy agents profile use work --print-env)"
```

With `--global`, symlink-tier agents are re-pointed atomically, the
registry records the active profile per agent, and `<name>` becomes the
default profile.

### `status`

```console
$ jarvy agents profile status --format json
```

Reports, per agent: mechanism (`env` / `symlink`), whether it is
switchable in v1, the active profile, and a state:

- `managed` — jarvy controls this agent's config (registry entry for the
  env tier; a store-pointing symlink for the symlink tier)
- `unmanaged` — a real directory (or a foreign symlink, e.g. from a
  dotfile repo) sits at the config path; a real directory is never
  clobbered by a switch
- `not_installed` — no config directory found

## The `jp` shorthand

The `jarvy shell-init` rc snippet (see [Task runner](run.md)) also
defines `jp`: bare `jp` lists profiles, `jp <name>` evals
`jarvy agents profile use <name> --print-env` into the current shell.
Supported in bash, zsh, sh, fish, PowerShell, and nushell.

## Configuration

```toml
# jarvy.toml
[agents.profiles]
prefer = "work"     # advisory profile hint for this project
strict = false      # v1: parsed, not yet enforced
```

The block is parsed in v1 (with the remote-config trust gate below);
setup-time hints that act on `prefer` land in v1.1.

## Trust boundaries

- **Store permissions.** `~/.jarvy/agent-profiles/` is created 0700 and
  the chmod is verified by read-back (`agent_profile.perms_unsafe` fires
  if a filesystem silently ignores it). Snapshots preserve file modes —
  credentials keep their 0600.
- **Unmanaged dirs are never clobbered.** If a real directory sits where
  jarvy expects its symlink (agent reinstalled itself), the switch is
  refused — move it aside or re-run `init`.
- **Home containment.** Symlink targets must canonicalize inside your
  home directory; anything else is refused.
- **Remote configs.** A config fetched via `jarvy setup --from <url>`
  cannot carry `[agents.profiles]` — the block is stripped
  (`agent_profile.remote_refused`).
- **Ticket bundles exclude the store.** `jarvy ticket create` never
  packs `~/.jarvy/agent-profiles/` (live credentials), and
  `CLAUDE_CONFIG_DIR` / `CODEX_HOME` are not in the ticket env
  allowlist.
- **Profile names never reach telemetry.** Events carry counts and
  bounded agent slugs only.

## v1 limitations

- `jarvy skills install` and `jarvy mcp register` write to the agent's
  *default* config path, not the env-redirected profile of the current
  terminal. Run them with the target profile active globally, or re-run
  after switching.
- No `save` verb yet — `init`/`create --from` snapshot; editing a live
  env-tier dir edits the snapshot it points at (env tier points directly
  into the store).
- No git sync between machines (planned v1.1).

## Telemetry

All events are gated on `[telemetry] enabled`. Profile names are never
emitted.

| Event | Notes |
|-------|-------|
| `agent_profile.created` / `agent_profile.deleted` | `agent_count` |
| `agent_profile.switched` | `agents` (slugs), `mechanism` (`env`/`symlink`), `scope` (`tty`/`global`), `duration_ms` |
| `agent_profile.unmanaged_dir` | switch refused over a real directory |
| `agent_profile.perms_unsafe` | store chmod failed or was ignored |
| `agent_profile.remote_refused` | remote config carried `[agents.profiles]` |
| `agent_profile.agent_absent` | snapshot skipped an uninstalled agent (debug) |

See `prd/058-agent-profile-switcher.md` (in the repo) for the full
design rationale.
