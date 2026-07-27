---
title: "Tool Freshness Advisory - Jarvy"
description: "Get a lightweight nudge when the dev tools your team pins in jarvy.toml have newer upstream versions available — no auto-upgrades, never blocks setup."
---

# Tool Freshness Advisory

Once you pin dev tools in `[provisioner]`, nothing tells you when those pinned versions fall behind upstream. Drift detection compares against the team baseline (see [drift.md](drift.md)); the freshness advisory compares against **upstream latest**.

The two answer different questions:

| Command | Question | Compares |
|---------|----------|----------|
| `jarvy drift check` | Is my machine still on the team baseline? | installed vs pinned baseline |
| `jarvy check-updates` | Is our pinned baseline falling behind upstream? | installed vs upstream latest |

Freshness is **advisory** — never auto-upgrades, never blocks `jarvy setup`. Its whole job is visibility.

## How It Works

`jarvy setup` reads `~/.jarvy/update-cache.json` (a local file — no network) and prints a one-line summary if any tools have newer versions. Then it spawns a detached background refresher that repopulates the cache for your **next** invocation. First-ever setup on a machine shows no summary because the cache is empty — the summary appears from run #2 onward.

On demand:

```bash
jarvy check-updates             # read cache, print report (fast)
jarvy check-updates --refresh   # foreground refresh, blocks ~5s × N tools
jarvy check-updates --format json
```

## Enabling / Disabling

Freshness is **on by default**. Opt out per project:

```toml
# jarvy.toml
[maintenance]
check_updates = false
```

Or globally via env var:

```bash
export JARVY_CHECK_UPDATES=0
```

## Configuration

```toml
[maintenance]
check_updates = true                    # default; false = disabled
cache_ttl_hours = 24                    # default; 0 = always refresh
ignore = ["docker", "kubectl"]          # per-tool opt-out
notify_on = "setup"                     # "setup" | "manual" | "never"
allow_remote = false                    # remote-config trust gate
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `check_updates` | bool | `true` | Master switch |
| `cache_ttl_hours` | int | `24` | Success entries expire after this many hours; error entries after `min(this, 1h)` |
| `ignore` | array | `[]` | Tools skipped entirely — useful for noisy nightly-release tools |
| `notify_on` | enum | `setup` | `setup` prints summary line; `manual` only surfaces via `jarvy check-updates`; `never` silences everything but keeps the cache warm |
| `allow_remote` | bool | `false` | Required for a `jarvy setup --from <url>` config to enable `check_updates = true` |

## What Gets Checked

The router iterates every configured tool and package section:

- `[provisioner]` — CLI tools installed via native package managers
- `[cargo]` — Rust binaries
- `[npm]` — Node globals
- `[pip]` — Python packages
- `[gem]` — Ruby gems
- `[go]` — Go modules
- `[nuget]` — .NET global tools

Tools installed via **version managers** (`rustup`, `nvm`, `pyenv`, `rbenv`, `sdkman`, `asdf`) and tools with **custom install scripts** are bucketed as `unchecked` — they sit off Jarvy's version-management path, and flagging them would create noisy false positives.

## Backends

Each package manager gets its own shell-out backend. No new HTTPS surface — if the manager isn't on `PATH`, the tool becomes `unchecked` (never falls back to a raw registry API).

| Backend | Command |
|---------|---------|
| brew | `brew info --json=v2 <formula>` |
| apt | `apt-cache policy <pkg>` |
| dnf | `dnf info --quiet <pkg>` (also serves yum) |
| pacman | `pacman -Si <pkg>` |
| apk | `apk search -x <pkg>` |
| winget | `winget show --id <id> --exact` |
| choco | `choco search <pkg> --exact --limit-output` |
| scoop | `scoop info <pkg>` |
| cargo | `cargo search <crate> --limit 1` |
| npm | `npm view <pkg> version --json` |
| pip | `pip index versions <pkg>` |
| gem | `gem search -re <pkg>` |
| go | `go list -m -versions -json <mod>@latest` |
| nuget | `dotnet tool search <id> --take 1` |

Each probe has a **5-second hard timeout** — a hung manager never wedges the report.

Per-OS routing preference (matches the installer's runtime picker so freshness answers reflect what `jarvy setup` would actually install):

- **macOS**: brew (formula → cask)
- **Linux**: apt → dnf → yum-via-dnf → pacman → apk → linuxbrew
- **Windows**: winget → choco → scoop

## Cache

Location: `~/.jarvy/update-cache.json` (chmod `0600` on Unix).

The cache is the whole reason setup stays fast. Successful lookups are held for `cache_ttl_hours` (24h default); backend errors are held for `min(ttl, 1h)` so a transient outage doesn't wedge the check for a full day.

Concurrent refreshers coordinate via `~/.jarvy/update-cache.lock` — if another `jarvy check-updates` is already running, the second exits with a debug event rather than trampling the cache. Stale locks (>1 hour, owner dead) are reclaimed automatically.

Delete `~/.jarvy/update-cache.json` at any time to force a fresh probe.

## CLI Reference

```
jarvy check-updates [--refresh] [--background]
                    [--only <tool>[,<tool>...]] [--ignore <tool>[...]]
                    [--include-unchecked] [--format json|pretty]
                    [--file ./jarvy.toml]
```

| Flag | Effect |
|------|--------|
| `--refresh` | Bypass cache; probe every backend fresh. Foreground blocks ~5s × N tools with a `Checking <tool>...` line per probe. |
| `--background` | Refresh silently and exit. Setup uses this to warm the cache without blocking your session. Implies `--refresh`. |
| `--only` | Restrict to a comma-separated tool list |
| `--ignore` | Skip a comma-separated tool list (layered over `[maintenance] ignore`) |
| `--include-unchecked` | Per-tool listing of the `unchecked` bucket (default is a single roll-up line) |
| `--format` | `pretty` (default) or `json` |

Exit codes:

| Code | Meaning |
|------|---------|
| `0` | No updates available (or trust-gate skipped) |
| `1` | Updates available |
| `2` | Config error |
| `3` | Every configured tool's backend is unavailable |

## Trust Boundaries

Freshness never installs anything, so the trust surface is small — but it *does* execute package-manager binaries, so the same rules apply as everywhere else in Jarvy:

- **Remote-config gate** — a `ConfigOrigin::Remote` config (loaded via `jarvy setup --from <url>`) may set `check_updates = false` (narrowing trust is fine) but cannot enable it without `[maintenance] allow_remote = true`.
- **Sandbox / CI auto-skip** — `sandbox::is_sandbox()` or `ci::is_ci()` skips the phase. Explicit CI opt-in via `JARVY_CHECK_UPDATES=1` in the workflow's env.
- **Non-TTY** — the setup path skips the background refresher spawn (cache reads still fire). Non-TTY setups won't have someone to see the next-run summary anyway.
- **No new HTTP surface** — every backend shells out to a package-manager binary that was already trusted enough to install software on this box. Freshness adds no new URLs, no new certs, no new supply-chain assumptions.

## Telemetry

All events are gated on `[telemetry] enabled` — users with telemetry off don't ship freshness event breadcrumbs even when the OTLP exporter is otherwise configured.

Events (`maintenance.*`):

- `phase_started` / `phase_completed` — lifecycle + counts
- `phase_skipped` — bounded reason label (`disabled_by_config`, `disabled_by_env`, `sandbox`, `ci`, `remote_refused`, …)
- `stale_tool` — per-tool advisory (tool, backend, installed, latest, direction)
- `background_spawned` / `background_spawn_failed` — setup phase detachment outcome
- `refresh_already_running` — lockfile guard fired
- `cache_read_failed` / `cache_write_failed` — best-effort cache I/O returned Err

Metric: `jarvy.maintenance.stale_tools` (gauge, tagged `machine_id + platform`). Enables the "how far behind is our fleet?" dashboard without any org needing to run its own scraper.

## Design Notes (Non-Goals)

The freshness advisory intentionally does **not**:

- **Auto-upgrade** anything. That's `jarvy setup` after you bump `[provisioner]`.
- **Consult CVE feeds.** Freshness ≠ security. A separate advisory pipeline is a possible follow-up.
- **Aggregate across projects.** Per-project / per-invocation only; org-wide rollups belong on the OTLP side.
- **Hit registries directly.** If `brew` isn't installed, brew-provisioned tools stay `unchecked` — never fall through to `api.brew.sh`.
- **Track version managers.** rustup / nvm / pyenv / rbenv / sdkman / asdf tools land in the `unchecked` bucket.

See `prd/057-tool-freshness-advisory.md` (in the repo) for the full design rationale.
