# PRD-058 — AI agent profile switcher

## Status

v1 implemented, unreleased. Storage + registry, the env tier
(claude-code, codex) and symlink tier (cursor),
`init/create/use/list/status/delete`, the `jp` shell shorthand,
gated telemetry, and the ticket-bundle exclusion all landed on the
`0.6.7` track. v1.1 (git sync, globalStorage, `save`/drift,
windsurf/cline/continue switching, the `prefer` setup hint) is
unstarted — see Phasing.

**Amended (2026-07-29), from dogfooding against a real home dir.**
"Snapshot the whole config dir" turned out to be unshippable as
written: `~/.claude` + `~/.codex` + `~/.cursor` measured 2.6 GB
against roughly 2 MB of actual identity, so every profile was a
multi-gigabyte copy and `create --from` cloned one identity's
conversation history into another. `src/agent_profiles/exclude.rs`
now denylists the non-identity bulk (transcripts, re-fetchable
package / extension / marketplace trees, log DBs, scratch), taking
this machine's `default` profile from 2.3 GB / 15 s to 2.9 MB /
0.9 s. Denylist rather than allowlist so an unrecognized new file is
kept — an oversized profile is a nuisance, a profile silently
missing config is a bug. Two other real-world defects surfaced the
same way: `fs::copy` aborts the entire snapshot on the live
`~/.codex/ipc/ipc.sock`, and `init` needed `--agents` because the
symlink tier moves a config dir out from under a running editor.

## Prior art

- [Mamdouh66/claude-switch](https://github.com/Mamdouh66/claude-switch) —
  bash account switcher for Claude Code only. Swaps credentials in
  under a second, stores them in the macOS Keychain, wraps the
  `claude` launcher behind a configurable shortcut (`c work`).
  Scope: credentials + token health display. No settings / skills /
  MCP isolation, single agent, macOS-centric.
- [guibes/claude-profile-switch](https://github.com/guibes/claude-profile-switch)
  (`cps`) — full profile isolation for Claude Code via the official
  `CLAUDE_CONFIG_DIR` env var. Each profile owns credentials,
  settings, skills, commands, agents, and MCP config. Git-backed
  with `age`-encrypted credentials, per-terminal activation through
  `eval "$(cps shell-init)"`, no file copying on switch.
  Scope: one agent (Claude Code), standalone tool.

Both tools prove the demand and the two viable mechanisms
(credential swap vs config-dir redirection). Neither covers more
than one agent, and neither integrates with a provisioning tool
that already knows where every agent's config lives.

## Problem

Developers increasingly hold multiple identities and configurations
per AI agent: a work account and a personal account, a
client-A-approved MCP allowlist vs a personal everything-enabled
setup, restricted skills for a compliance-bound repo vs a
kitchen-sink home profile. Today switching means logging out and
back in (Claude Code), hand-editing `~/.codex/config.toml` (Codex),
or maintaining shell aliases that export env vars per terminal —
per agent, with no consistency across the six agents Jarvy already
manages (claude-code, cursor, codex, windsurf, cline, continue).

Jarvy is uniquely positioned: `src/agents.rs` already enumerates
the agents, and the `ai_hooks` / `mcp_register` / `skills`
subsystems already know each agent's on-disk config layout. A
profile switcher is the missing lifecycle verb on top of the
provisioning Jarvy already does.

## Goals

- `jarvy agents profile {init,create,list,use,save,delete,status}`
  — one CLI surface that manages named profiles across all
  supported agents.
- **On-the-fly, per-terminal switching** where the agent supports
  config-dir redirection (env var), falling back to an explicit
  global swap where it doesn't. Two terminals can run `work` and
  `personal` simultaneously for env-redirect agents.
- Profile = the agent's whole config dir (credentials, settings,
  skills, MCP registrations, memory/instructions), not just
  credentials. Isolation prevents cross-profile bleed of MCP
  servers and skills — the same properties `cps` demonstrates.
- Integrate with the existing `jarvy shell-init` snippet (PRD run
  infra) so `use` can rewrite the current shell's env without a
  new shell entry point.
- Per-project pinning: a `[agents.profiles]` block in `jarvy.toml`
  can declare "this repo wants profile `client-a`", surfaced (never
  auto-applied from remote configs) during `jarvy setup`.
- All six `Agent` variants supported from day one at the storage
  level; switching mechanics tiered per agent (see table).
- **Switch awareness** — jarvy (and the agents themselves, via a
  distributed skill) know *when* a switch is warranted, not just
  how to perform one. Directory-entry hints, setup hints, and an
  agent-facing SKILL.md close the loop so the user doesn't have to
  remember which repo wants which profile.
- **Git-backed sync** — profiles are versioned in a local git repo
  with `age`-encrypted credential files, pushable to a private
  remote so a profile set follows the user across machines (the
  `cps` model, generalized to all agents).
- **IDE globalStorage coverage** — for the VS Code-family agents
  (cursor, windsurf, cline, continue), a profile also captures the
  extension's globalStorage slice under the IDE's user-data dir,
  not just the `~/.{agent}/` dotdir.

## Non-goals

- **No credential brokering.** Jarvy never touches auth flows,
  refreshes tokens, or parses credential formats. It moves/points
  at opaque agent-owned files (encrypting them at rest for git sync
  is storage, not brokering). Token health display à la
  `claude-switch` is out of scope for v1.
- **No daemon, no background watcher.** Considered and rejected.
  What a daemon would buy: auto-switching on `cd` without shell
  hooks, watching IDE process lifecycles to time globalStorage
  swaps, proactive token refresh. None of it pays for itself: the
  shell-init hook already covers cwd-based awareness at zero
  resident cost, IDE-lifecycle timing is handled by an explicit
  running-process check at switch time, and token refresh is
  credential brokering (see above). What it would cost: a
  long-lived process with read access to every profile's
  credentials (a materially worse compromise target than a
  short-lived CLI), install/upgrade/crash lifecycle management on
  three platforms, and a violation of the existing posture — the
  PRD-057 background refresher was deliberately spawn-and-exit for
  the same reason. Switching stays explicit; awareness comes from
  hooks and hints.
- **No in-loop skill optimization (SkillOpt et al.).** Tools like
  [microsoft/SkillOpt](https://github.com/microsoft/SkillOpt)
  evolve SKILL.md content through a rollout/reflect/update training
  loop (including a nightly `skillopt-sleep` self-evolution engine).
  Jarvy will not embed or invoke that loop — it's a long-lived
  optimizer process (the daemon rejection above applies) and it
  would make jarvy a *generator* of skill content rather than a
  verifier/distributor of it. The supported integration is
  producer/consumer: a publisher runs SkillOpt offline, ships the
  winning `best_skill.md` as the next skill version in a
  library_sources manifest (version bump + sha pin), and consumers
  pick it up through the existing `jarvy skills update` path —
  sidecar version/sha divergence reinstalls consistently across all
  agents, pinned mismatches still refuse. Optional follow-up under
  PRD-049: advisory provenance fields in the manifest (`optimizer`,
  eval score) surfaced by `jarvy skills status`; the sha pin stays
  the trust anchor. See `docs/skill-optimization.md`.

## Design

### Storage layout

```
~/.jarvy/agent-profiles/
  profiles.json                  # registry: active profile per agent + per-tty overrides
  <profile-name>/
    claude-code/                 # a full CLAUDE_CONFIG_DIR
    codex/                       # a full CODEX_HOME
    cursor/
    windsurf/
    cline/
    continue/
```

- Dir mode 0700, files preserved as-is (credentials keep their
  original modes). Mirrors the `discover.jarvy_toml_perms_unsafe`
  chmod-verify pattern: emit `agent_profile.perms_unsafe` if the
  chmod is refused or silently ignored.
- `profiles.json` is jarvy-owned state, schema-versioned like
  `update-cache.json`.
- Profile names validated with the same grammar as
  `validate_skill_name` (no traversal, no control bytes).

### Switching mechanics — two tiers

| Agent | Mechanism | On-the-fly (per-terminal)? |
|---|---|---|
| claude-code | `CLAUDE_CONFIG_DIR=<profile>/claude-code` (official env var, proven by `cps`) | Yes |
| codex | `CODEX_HOME=<profile>/codex` (verify against current Codex CLI docs at implementation time) | Yes |
| cursor | symlink swap: `~/.cursor` → profile dir + globalStorage swap | No — global, all terminals |
| windsurf | symlink swap + globalStorage swap | No |
| cline | globalStorage swap (extension state lives in the host IDE) | No |
| continue | symlink swap (`~/.continue`) + globalStorage swap | No |

- **Env tier**: `jarvy agents profile use work` prints (or, via the
  `shell-init` wrapper function, `eval`s) the export lines for the
  current shell. Nothing global changes; each terminal can point at
  a different profile. `profiles.json` records the default so new
  shells auto-activate it through the rc snippet.
- **Symlink tier**: the real dir is moved into the profile store
  once (`init`/`save`), and `~/.cursor` becomes a symlink into the
  active profile. `use` re-points the symlink atomically
  (rename-over). Every switch is global for that agent; the CLI
  says so explicitly. If the path is a real directory with local
  changes (user reinstalled the agent), refuse with a
  `agent_profile.unmanaged_dir` error instead of clobbering —
  investigate-don't-delete, same posture as the git-hooks installer.
- Windows: symlink tier uses directory junctions (no admin
  required); env tier unchanged.

### IDE globalStorage swap

The VS Code-family agents keep part of their state outside the
dotdir, in the host IDE's user-data tree:

```
macOS:   ~/Library/Application Support/<IDE>/User/globalStorage/<ext-id>/
Linux:   ~/.config/<IDE>/User/globalStorage/<ext-id>/
Windows: %APPDATA%\<IDE>\User\globalStorage\<ext-id>\
```

where `<IDE>` ∈ {`Cursor`, `Windsurf`, `Code` (for cline /
continue running in VS Code)} and `<ext-id>` is the extension's
publisher.name dir (e.g. cline's). A profile snapshot for these
agents captures **both** the dotdir and the globalStorage slice;
`use` swaps both or neither (single logical transaction — if the
second swap fails, the first is rolled back).

Constraints:

- **IDE must be closed.** globalStorage contains SQLite/LevelDB
  state the IDE holds open; swapping under a running IDE corrupts
  or silently loses writes. `use` runs a process check per
  affected IDE and **refuses** (not warns) when it's running,
  emitting `agent_profile.ide_running { ide }`. `--force` exists
  for the user who knows the IDE is a zombie process.
- Only the *extension's* globalStorage dir is swapped — never the
  whole IDE profile (themes, editor settings, unrelated extensions
  stay put).
- Exact ext-id per agent is pinned in `Agent::profile_mechanism()`
  alongside the dotdir path, so it lives in the one canonical enum.
- Portable-mode / custom `--user-data-dir` installs aren't detected
  in v1; `status` reports the globalStorage slice as `not_found`
  rather than guessing.

### Knowing when to switch

Three escalating layers of awareness — jarvy never switches on its
own (credential movement stays behind an explicit user action), but
it makes the *need* to switch impossible to miss:

1. **Directory-entry hint (shell hook).** The `shell-init` snippet
   gains a lightweight cwd hook (`chpwd` on zsh, `PROMPT_COMMAND`
   on bash, `fish_prompt` event): on entering a directory whose
   `jarvy.toml` carries `[agents.profiles] prefer`, compare against
   the active profile (one read of `profiles.json`, no subprocess
   in the fast path — the snippet checks a mtime-cached state file)
   and print a one-line nudge: `jarvy: this repo prefers agent
   profile 'client-a' (active: personal) — run 'jp client-a'`.
   Silent when they match. Debounced per (tty, repo) so it fires
   once per session, not per prompt.
2. **Setup hint.** `jarvy setup` compares active vs `prefer` and
   prints the same nudge (warning-level with `strict = true`).
   Already covered by `[agents.profiles]` below.
3. **Agent-side skill.** A `jarvy-profile-awareness` SKILL.md
   distributed through the existing skills infrastructure (PRD-049
   pipeline — same install path, same sidecar) teaches the *agent*
   to run `jarvy agents profile status --format json` at session
   start and, when the repo's preferred profile isn't active, tell
   the user and suggest the switch command. The agent can also
   recognize context clues the shell can't ("this looks like
   client work but I'm running with your personal MCP servers").
   The skill only ever *suggests* — the actual `use` needs the
   user's shell because env-tier switching must happen in their
   terminal, and because auto-switching credentials from inside an
   agent session is exactly the confused-deputy scenario the
   remote-config gate exists to prevent.

### Git sync

Profiles are a git repo. `agent-profiles/` is `git init`-ed on
`profile init`; every `create` / `save` / `delete` auto-commits
(message = the CLI invocation, timestamps in the commit). This is
in scope for v1.1, not deferred to a separate PRD:

- **Credentials never land in git plaintext.** Files matching each
  agent's credential globs (`.credentials.json`, `auth.json`, …
  pinned per-agent in `profile_mechanism()`) are `age`-encrypted
  into `<name>.age` blobs before commit; the plaintext lives only
  in the working tree and is `.gitignore`-d. The `age` identity key
  lives at `~/.jarvy/agent-profiles/.age-key` (0600, never
  committed) — `jarvy agents profile init --key <path>` imports it
  on a second machine, mirroring the `cps` flow.
- `jarvy agents profile sync [--remote <url>]` — sets/uses a
  remote, `pull --rebase` then push. Conflicts stop with git's
  own conflict state and a pointer to the dir; jarvy does not
  auto-resolve profile conflicts.
- Remote must be HTTPS or SSH to a host the user configures
  explicitly. Jarvy never suggests or defaults a remote.
- Rollback for free: `status` prints the head commit; `jarvy
  agents profile restore <name> [--rev <sha>]` checks a profile
  dir out of history.
- `git` missing from PATH → sync features report
  `library.git.missing_git`-style and everything local still
  works. No new git implementation — shell out like
  `library_registry` does.
- The globalStorage slice syncs like any other profile content;
  LevelDB/SQLite binaries are committed as opaque blobs (they're
  small; content-diffing them is a non-goal).

### CLI surface

```
jarvy agents profile init                      # snapshot current configs as 'default'
jarvy agents profile create <name> [--from <p>] [--agents claude-code,codex]
jarvy agents profile use <name> [--agents ...] [--global] [--force] [--print-env]
jarvy agents profile save [<name>]             # re-snapshot live config into profile
jarvy agents profile list [--format json]
jarvy agents profile status [--format json]    # active per agent, per-tty overrides, drift
jarvy agents profile delete <name>
jarvy agents profile sync [--remote <url>]     # commit + optional push (see Git sync)
jarvy agents profile restore <name> [--rev <sha>]
```

- `use` without `--global` affects env-tier agents in the calling
  shell only (via shell-init integration) and prints a notice for
  symlink-tier agents that a global swap will occur; `--global`
  applies everything and updates the default in `profiles.json`.
- `status` reports per-agent: active profile, mechanism, and
  managed/unmanaged/not-installed state. Drift reporting (whether
  the live dir has diverged from the last `save`) lands in v1.1
  alongside `save` itself — drift is defined relative to a save
  baseline, so it cannot ship before `save` does. v1.1 uses an
  mtime + file-count heuristic; content hashing is phase 2.
- `--force` overrides the running-IDE refusal for globalStorage
  swaps (see "IDE globalStorage swap"); `--print-env` emits the
  env-tier exports for shell eval (consumed by `jp`).
- `--format json` rides the PRD-051 `Outputable` pattern.
- `--agents` narrows to a subset; names reuse the canonical
  `agents::Agent` slugs so a new enum variant lands here
  automatically.

### Shell integration

Extend the existing `jarvy shell-init` snippet (see `run_cmd.rs` /
`shell-init --apply` plumbing) with a `jarvy-profile` (alias `jp`)
function: `jp work` → runs `jarvy agents profile use work
--print-env` and evals the output. Same nushell materialization
path as the `jr` shorthand. Bare `jp` lists profiles.

### Config block (project-level pinning)

```toml
[agents.profiles]
prefer = "client-a"      # suggested profile for this repo
strict = false           # true: setup warns loudly when active != prefer
```

- Advisory only: `jarvy setup` compares active profile vs `prefer`
  and prints a hint (or a warning with `strict = true`). It never
  auto-switches — switching changes credentials, and credentials
  never move without an explicit user command.
- **Remote-config gate**: `ConfigOrigin::Remote` configs cannot
  declare `[agents.profiles]` at all — a hostile remote config
  suggesting a profile switch is a credential-redirection primitive.
  Refused with `agent_profile.remote_refused { reason =
  "remote_origin" }`, consistent with the library_sources refusal.

### Trust boundaries

- Profiles may contain live credentials. Everything under
  `agent-profiles/` is 0700, never logged by content, and profile
  *names* are the only field that reaches telemetry — after the
  same bounded-label scrutiny as `workspace.member_invalid`
  (names are user-authored → log counts and hashes, not names).
- `jarvy ticket create` (debug bundles) must exclude
  `agent-profiles/` entirely — add to the ticket collector's
  denylist in the same change.
- The symlink swap refuses to operate on paths that resolve outside
  `$HOME` (reuse `safety::resolve_within_workspace` shape against a
  home-rooted boundary).
- No `sudo`, no elevation. If a config dir isn't writable, report.

### Telemetry (gated, per convention)

| Event | Fields |
|---|---|
| `agent_profile.created` / `deleted` / `saved` | `agent_count`, `mechanism_counts` |
| `agent_profile.switched` | `agents`, `mechanism = "env" \| "symlink" \| "globalstorage"`, `scope = "tty" \| "global"`, `duration_ms` |
| `agent_profile.remote_refused` | `reason` |
| `agent_profile.unmanaged_dir` | `agent` |
| `agent_profile.perms_unsafe` | `fs_hint` |
| `agent_profile.setup_hint` | `strict`, `matched` (bool) |
| `agent_profile.ide_running` | `ide`, `forced` (bool) — globalStorage swap hit a live IDE; refused unless `--force` |
| `agent_profile.cwd_hint` | `matched` (bool) — shell-hook awareness fired (debounced; no profile names) |
| `agent_profile.synced` | `pushed` (bool), `encrypted_file_count`, `duration_ms` |
| `agent_profile.sync_failed` | `error_kind = "git_missing" \| "conflict" \| "auth" \| "network" \| "encrypt" \| "io"` — never raw git stderr (repo URL may embed tokens, same rule as `dotfiles.phase_failed`) |
| `agent_profile.restored` | `rev_specified` (bool), `duration_ms` |

All routed through `observability::telemetry_gate::is_enabled()`.

### Module map

```
src/agent_profiles/
  mod.rs         # public API + ProfileRegistry (profiles.json)
  store.rs       # snapshot/restore, chmod verify, name validation
  switcher.rs    # env-tier emit + symlink-tier atomic re-point
  globalstorage.rs # IDE globalStorage slice swap + running-IDE probe
  status.rs      # drift heuristic, per-agent report
  sync.rs        # git shell-out, age encrypt/decrypt, restore
src/commands/agents_profile_cmd.rs
```

`agents::Agent` gains a `profile_mechanism()` -> `Env { var } |
Symlink { home_dir } | GlobalStorage { ide, ext_id }` accessor so
per-agent knowledge stays in the one canonical enum
(VS Code-family agents return both `Symlink` for their home dir
slice and `GlobalStorage` for the IDE slice — the accessor
returns a small `&[Mechanism]`).

## Phasing

1. **v1**: storage + registry, claude-code (env) + codex (env) +
   cursor (symlink), `init/create/use/list/status/delete`,
   shell-init `jp`, telemetry, ticket-bundle exclusion.
2. **v1.1**: git sync (`sync` / `restore`, `age`-encrypted
   credential globs, remote push), IDE globalStorage swap for the
   VS Code-family agents (running-IDE refusal + `--force`),
   windsurf / cline / continue symlink tier, `save` drift
   detection, `[agents.profiles]` setup hint + cwd shell hook.
3. **v2 (separate review)**: `jarvy-profile-awareness` skill via
   the PRD-049 pipeline, profile templates from library_sources,
   token-health display.

## Open questions

- ~~`CODEX_HOME` is verified present in the openai/codex source, but
  re-confirm against the shipping CLI release before v1 lands.~~
  **Resolved (2026-07-29).** Confirmed against shipping `codex-cli
  0.135.0`: `CODEX_HOME=<dir> codex doctor` reports `CODEX_HOME
  <dir> (dir)` and resolves `auth file` to `<dir>/auth.json`, and
  the run materializes `memories/` + `tmp/` under the override. The
  standalone-release `packages/` tree stays at the real `~/.codex`
  (it's the installed runtime, not per-account state), which is the
  behavior we want — profiles swap identity and config, not the
  binary.
- Should `use --global` for a *symlink-tier* agent (CLI-only, no
  IDE) also refuse while the agent process is running (lsof/process
  check), or warn only? The globalStorage tier already refuses
  (with `--force`); extending the probe to CLI agents is cheap but
  may be noisy for long-lived TUI sessions.
- `age` key distribution across machines: v1.1 requires manual
  `init --key` import; is a passphrase-derived key worth offering
  as an alternative?
