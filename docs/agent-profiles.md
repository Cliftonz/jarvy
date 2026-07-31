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

| Tier | Agents | Mechanism | Scope |
|------|--------|-----------|-------|
| Env | claude-code (`CLAUDE_CONFIG_DIR`), codex (`CODEX_HOME`) | export an env var pointing into the profile store | **Per terminal** — two shells can run different profiles at once |
| Symlink | cursor, windsurf, cline, continue | `~/.{agent}` becomes a symlink into the profile store, atomically re-pointed | **Global** — all terminals and the IDE see the switch |

All six agents are switchable. Symlink-tier swaps are refused when the
target IDE is running unless `--force` is passed (see `use`). Cline and
Continue live inside VS Code / Cursor as extensions and have no separate
process to probe — swaps go through without the check. Git-based sync
between machines is still v1.1.

## Commands

```
jarvy agents profile init    [--agents a,b]    # snapshot installed agents as 'default'
jarvy agents profile create  <name> [--from <src>] [--agents a,b]
jarvy agents profile use     <name> [--agents a,b] [--global] [--force] [--print-env]
jarvy agents profile save    [name] [--agents a,b]      # refresh the snapshot from live
jarvy agents profile restore <name> [--agents a,b]      # copy the snapshot back to live
jarvy agents profile list    [--format json|pretty]
jarvy agents profile status  [--format json|pretty]
jarvy agents profile delete  <name>
```

### `init`

Snapshots every installed agent into a profile named `default` and sets
it as the default. Env-tier agents are **copied** (the live directory
keeps working untouched); symlink-tier agents are **moved** into the
store with a symlink left at the original path. Idempotent — re-running
skips agents that are already managed.

`--agents claude-code,codex` narrows the snapshot. Use it when a
symlink-tier editor is open: that tier *moves* the live config directory,
which a running IDE will not appreciate. A narrowed run only speaks for
the agents it named, so it leaves an already-set default profile alone —
it still claims `default` when the store has none, otherwise `use` would
have nothing to fall back on.

## What a profile does and does not contain

A profile carries **identity**: credentials, `settings.json` /
`config.toml`, `CLAUDE.md` / `AGENTS.md`, skills, MCP registrations,
plugin selections.

It deliberately skips what an agent directory merely accumulates —
conversation transcripts (`~/.claude/projects`), re-downloadable package
and marketplace trees (`~/.codex/packages`, `~/.claude/plugins/cache`),
log databases and scratch space. On one real machine that is the
difference between 2.3 GB and 2.9 MB per profile. `create --from` runs
the same filter, resolving each top-level directory in the source profile
to its agent, so one identity's conversation history never seeds
another's.

The rule set (`src/agent_profiles/exclude.rs`) is a denylist: a file
jarvy does not recognize is **kept**. A profile being larger than
necessary is a nuisance; a profile silently missing config you rely on is
a bug. Two things are never filtered:

- **The symlink tier's move.** Relocating a directory deletes the source
  once the copy lands, so a skipped file would be destroyed rather than
  left behind. The code carries this as a type (`CopyPolicy::Relocate`),
  not a comment.
- **Cursor's `extensions` tree**, despite being re-downloadable. Cursor
  is symlink-tier: its snapshot *is* the live config directory, so a
  profile without extensions is an editor without extensions, and
  nothing re-installs them.

Live IPC endpoints such as `~/.codex/ipc/ipc.sock` are skipped by file
type — they are not copyable, and one of them used to fail the whole
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
default profile. If the target IDE is running, the swap is refused for
that agent and one `agent_profile.ide_running` event fires; pass
`--force` to override and see one stderr line reminding you to restart
the editor.

### `save`, `restore`

`save [name]` refreshes the snapshot from the live config directory.
Env-tier agents get their live dir re-copied over the snapshot; symlink
agents are already live-in-profile (their `~/.slug` is a jarvy-managed
symlink into the store), so their `save` is a no-op with a note. Without
`name`, each agent saves to whichever profile it is currently on — env
tier reads its `CLAUDE_CONFIG_DIR` / `CODEX_HOME`, symlink tier reads
the live link.

`restore <name>` is the inverse: env-tier snapshots are copied back to
`~/.claude` / `~/.codex`, so a fresh shell (no env vars) sees the
restored profile; symlink-tier agents are re-pointed exactly as
`use --global` does. Refuses when the profile has no snapshot for a
targeted agent.

### `check-cwd`

Fires from the shell `cd` hook installed by `jarvy shell-init` (bash /
zsh / fish). Walks up from cwd for a `jarvy.toml`; if `[agents.profiles]
prefer` is set and the live profile diverges, prints one stderr line.
Debounced per (session, repo) for 4 h via a 0600 state file at
`~/.jarvy/agent-profiles/.cwd-hint-state.json` — the same repo in a
sibling terminal still gets nudged once. Opt out with
`JARVY_NO_CWD_HINT=1`.

### `status`

```console
$ jarvy agents profile status --format json
```

Reports, per agent: mechanism (`env` / `symlink`), whether it is
switchable, the active profile, and a state:

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
strict = false      # true = warn on stderr instead of a soft hint
```

`jarvy setup` compares each switchable agent's live profile against
`prefer` and prints a hint when they diverge. It never auto-switches:
switching a profile swaps the credentials an agent authenticates with,
and credentials don't move without an explicit user command. `strict =
true` only raises the volume — a stderr warning rather than a stdout
hint. Neither fails setup; a wrong-profile machine is something to tell
you about, not a reason to refuse to install your tools.

Two cases are kept apart. An agent sitting on a *different* profile is
a mismatch and triggers the hint. An agent that's installed but not
profile-managed at all is merely unmanaged — the repo asked for a
profile and you aren't using profiles, so there's nothing to correct.
Uninstalled agents never appear; the hint would be telling you to
switch something you don't have.

For the env tier the current terminal wins: `prefer` is compared
against `CLAUDE_CONFIG_DIR` / `CODEX_HOME`, not `profiles.json`, which
only records the last *global* switch. Switch in one shell and that
shell stops nagging while the others carry on.

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

## Known limitations

- `jarvy skills install` and `jarvy mcp register` write to the agent's
  *default* config path, not the env-redirected profile of the current
  terminal. Run them with the target profile active globally, or re-run
  after switching.
- No git-backed sync between machines yet (planned v1.1) — a profile
  set doesn't follow you to a second laptop without copying the store
  by hand.
- No drift detection sidecar on snapshots (`save` refreshes; `restore`
  overwrites; there's no "since you last saved this profile, N files
  changed on disk" report).

## Telemetry

All events are gated on `[telemetry] enabled`. Profile names are never
emitted.

| Event | Notes |
|-------|-------|
| `agent_profile.created` | `action` (`init`/`create`), `seeded_from_existing`, `agents_filtered` (was `--agents` passed), `agent_count`, `duration_ms` |
| `agent_profile.deleted` | `agent_count` |
| `agent_profile.switched` | one row per agent: `agent` (slug), `mechanism` (`env`/`symlink`), `scope` (`tty`/`global`), `agents_filtered`, `first_switch`, `invocation_source` (`rc_snippet`/`manual`), `duration_ms`. Symlink rows are emitted the moment the link is re-pointed, so a later registry-write failure can't erase the record of a switch the editor already sees |
| `agent_profile.snapshot_completed` | `agent`, `mode` (`copy`/`move`), then totals: `files_copied`, `bytes`, `subtrees_excluded`, `symlinks_skipped`, `special_skipped`, `duration_ms`. The per-path skips below are `debug`, so this aggregate is what makes a denylist that stopped matching visible. `subtrees_excluded` counts denylist *matches*, not files — an excluded directory is skipped whole, so one match can stand for a gigabyte |
| `agent_profile.snapshot_failed` | `agent`, `mode`, `error_kind`, `duration_ms`. Which agent aborted the run exists only here — `op_failed` knows the subcommand, not the agent, and a `move` that died mid-copy has already deleted part of the source |
| `agent_profile.clone_completed` | `create --from` roll-up across every agent in the profile: `files_copied`, `bytes`, `subtrees_excluded`, `symlinks_skipped`, `special_skipped`, `duration_ms`. No `agent` field — filing it under a sentinel would put a value in that dimension no per-agent query can mean |
| `agent_profile.op_failed` | `action` (the subcommand), `stage` (the step within it — `snapshot`, `registry_save`, `symlink_repoint`, …), `error_kind`. `stage` is what separates a failed snapshot copy from a failed registry write; only the latter leaves the store half-applied |
| `agent_profile.saved` | `agent_count` (sum, kept for backwards compat), `env_copied`, `symlink_noop`, `skipped_count` (agents with no active profile), `agents_filtered`, `duration_ms` — `save` refreshed the named profile (or each agent's active profile when unspecified). Split fields mirror `skills.updated` so "everything ran cleanly" is distinguishable from "half the agents were skipped for reasons". Profile names never emitted |
| `agent_profile.restored` | `agent_count` (sum), `env_copied`, `symlink_repointed`, `skipped_count` (unfiltered runs silently skip agents lacking a snapshot; the running-IDE guard's per-agent refusals also increment this — explicit `--agents` still errors on a missing snapshot), `agents_filtered`, `duration_ms` — `restore` copied a profile back over the live config dirs (env tier) and re-pointed the symlink tier |
| `agent_profile.ide_running` | `agent` (slug), `forced` (bool) — the target IDE was running when a symlink-tier swap was attempted. `forced = false` means the swap was refused (warn); `forced = true` means `--force` overrode (info) |
| `agent_profile.cwd_hint` | `matched`, `debounced`, `invocation_source = "rc_snippet" \| "manual"` (from `JARVY_CWD_HINT_INVOCATION`) — the shell `cd` hook fired for a repo carrying `[agents.profiles] prefer`. Debounced runs are silent; only non-debounced runs emit. No profile names, no path |
| `agent_profile.use_noop` | `reason = "symlink_tier_needs_global"`, `agents` — `use` selected symlink-tier agents without `--global`, so nothing switched for them |
| `agent_profile.unmanaged_dir` | switch refused over a real directory |
| `agent_profile.perms_unsafe` | store chmod failed or was ignored |
| `agent_profile.remote_refused` | remote config carried `[agents.profiles]` |
| `agent_profile.not_configured` | `jarvy setup` saw no `profiles.json` — the adoption-funnel denominator. Debug |
| `agent_profile.setup_hint` | `strict`, `matched`, `mismatched_count`, `unmanaged_count` — `jarvy setup` compared the live profile against `[agents.profiles] prefer`. Profile names are not emitted |
| `agent_profile.agent_absent` | snapshot skipped an uninstalled agent (debug) |
| `agent_profile.snapshot_cross_device` | `agent` — a move-mode snapshot hit EXDEV and fell back to copy-then-delete. Debug; warn (with `error_kind`, `error`) when the post-copy source delete failed |
| `agent_profile.path_excluded` | `agent`, `pattern` (the denylist rule that fired — a jarvy constant; the path is not emitted). Debug |
| `agent_profile.symlink_skipped` | `agent`, `reason` (`absolute_target`/`escapes_root`/`unreadable`). The path is not emitted — under an agent config dir it would carry `$HOME`, which on macOS/Windows is the account name |
| `agent_profile.special_file_skipped` | `agent`, `kind`, `relocating`. Warn when relocating (the source is about to be deleted), debug otherwise |

The `debug`-level rows above don't reach `~/.jarvy/logs/jarvy.log` by
default — the profile subcommands build no `ObservabilityConfig`, so
`-v` doesn't widen them. Use `RUST_LOG=jarvy=debug jarvy agents profile
init <name>` to see per-path exclusions and symlink skips.

See `prd/058-agent-profile-switcher.md` (in the repo) for the full
design rationale.
