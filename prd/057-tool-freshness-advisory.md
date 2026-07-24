# PRD-057 — Tool freshness advisory

## Status

Implemented. Phase 1 (brew/cargo/npm + setup wiring) shipped in
`0.6.7`-track work; phase 2 (apt/dnf/winget/choco/scoop + pip/gem/go
backends, `npm ls -g` installed detection, lockfile guard) landed
right after in the same iteration.

## Problem

Jarvy pins dev tools in `[provisioner]`. Once installed, no signal
tells the maintainer that a pinned tool has moved on upstream. Drift
detection (PRD-047-era `jarvy drift`) only compares installed vs the
pinned baseline in `.jarvy/state.json` — it answers "does my machine
match the config?", not "is the config itself falling behind?". The
gap shows up as stale toolchains that quietly rot until a security
advisory, a CI break, or a new-hire onboarding surfaces them.

Maintainers want a low-effort nudge: "N of your tools have newer
upstream versions available." No forced upgrades — just visibility, so
version bumps become a maintenance chore rather than a firefight.

## Goals

- New advisory surface: `jarvy check-updates` reports a per-tool
  installed-vs-upstream-latest comparison across every `[provisioner]`
  entry.
- Setup integration: `jarvy setup` **never blocks on freshness
  checks**. It reads the local cache (zero network) to print a
  one-line summary and spawns a detached background refresher that
  populates the cache for the *next* invocation. First-ever run
  shows no summary; second run onward shows the previous run's
  results. Advisory only — never blocks the run, never delays exit.
- On by default, opt-out per project via `[maintenance] check_updates
  = false`. Global env kill-switch `JARVY_CHECK_UPDATES=0`.
- Package-manager-native lookups (`brew info`, `cargo search`, `npm
  view`, `apt-cache policy`, `dnf info`, `winget show`) — no
  jarvy-hosted metadata index, no new supply-chain surface.
- Result cache at `~/.jarvy/update-cache.json` with a 24 h TTL so
  daily `setup` runs cost one query per tool per day, not per
  invocation.

## Non-goals

- No auto-upgrade. `check-updates` is read-only; upgrading is the
  maintainer's call and happens via `jarvy setup` after they bump
  `[provisioner]`.
- No CVE / advisory feed integration in v1. Freshness ≠ security; a
  security advisory pipeline is a separate PRD.
- No cross-project aggregation dashboard. Advisory is per-project /
  per-invocation; org-wide rollups belong on the OTLP side (event
  taxonomy below is designed to make that possible externally).
- No network calls without a package manager on PATH — Jarvy will not
  hit registry APIs (crates.io, npm registry, GitHub Releases)
  directly in v1. If `brew` isn't installed, brew-provisioned tools
  are marked `unchecked`, not fetched via HTTPS. Keeps the trust
  surface identical to `setup`.
- No handling for tools installed via version managers (`rustup`,
  `nvm`, `pyenv`, `rbenv`, `sdkman`, `asdf`). Same skip list as
  `drift::detector::is_auto_fixable`. Their upstream cadence is
  outside Jarvy's control and the check would be noisy.
- No handling for tools with `custom_install` scripts (nvm / rustup
  / brew bootstrap). Marked `unchecked` with a stable reason label.

## Design

### New module: `src/maintenance/`

```
src/maintenance/
  mod.rs           # Public API: check_updates(config, cache_dir) -> Report
  config.rs        # MaintenanceConfig ([maintenance] TOML block)
  checker.rs       # Orchestrator: iterate provisioner tools, fan out to backends
  backends/        # One file per package manager
    brew.rs
    cargo.rs
    npm.rs         # covers pip / gem / go via the shared registry-CLI shape
    apt.rs
    dnf.rs
    winget.rs
  cache.rs         # ~/.jarvy/update-cache.json read/write + TTL check
  reporter.rs      # Human + JSON output (Outputable impl)
```

### Config block

```toml
[maintenance]
check_updates = true          # default; false disables
cache_ttl_hours = 24          # default; 0 = always refresh
ignore = ["docker", "kubectl"] # per-tool opt-out
notify_on = "setup"           # "setup" (default) | "manual" | "never"
```

Placed in the canonical `TOP_LEVEL_SECTIONS` const in `config.rs` and
covered by the existing `top_level_sections_matches_config_fields`
destructure test.

### Trust boundaries

- **Remote-config gate** — a `ConfigOrigin::Remote` config MAY set
  `check_updates = false` (narrowing trust is always allowed) but a
  remote config declaring `check_updates = true` on a project that
  had it disabled MUST be refused with `maintenance.remote_refused
  { reason = "allow_remote_not_set" }`, mirroring
  `[packages] allow_remote`. Add `allow_remote = false` field.
- **Sandbox / CI auto-disable** — `sandbox::is_sandbox() = true` or
  `ci::is_ci() = true` skips the phase (matches
  `services::daemon_check` conventions). Users who want it in CI set
  `JARVY_CHECK_UPDATES=1`.
- **No new HTTP surface** — every backend shells out to an already-
  trusted package-manager binary. Zero new URLs, zero new certs, zero
  new supply-chain assumptions.
- **Cache path** — `~/.jarvy/update-cache.json`, mode 0600. Corrupt
  cache is logged (`maintenance.cache_read_failed`) and treated as a
  miss — never fatal.

### Backend contract

Each backend implements:

```rust
pub trait FreshnessBackend {
    fn name(&self) -> &'static str;
    fn latest(&self, pkg_id: &str) -> Result<String, BackendError>;
}
```

Command shape per backend (parsers live next to the shell-out):

| Backend | Command | Parse target |
|---|---|---|
| brew | `brew info --json=v2 <formula>` | `.formulae[0].versions.stable` |
| cargo | `cargo search <crate> --limit 1` | `"<crate> = "<ver>""` line |
| npm | `npm view <pkg> version --json` | root JSON string |
| pip | `pip index versions <pkg>` (fallback: `pip install <pkg>== 2>&1 \| head`) | Latest line |
| gem | `gem search -re <pkg>` | `<pkg> (<ver>)` line |
| go | `go list -m -versions -json <mod>@latest` | `.Versions[-1]` |
| apt | `apt-cache policy <pkg>` | `Candidate:` |
| dnf | `dnf info <pkg>` | `Version : <ver>` (last block wins) |
| winget | `winget show --id <id> --exact` | `Version:` line |
| choco | `choco search <id> --exact --limit-output` | `<id>\|<ver>` |
| scoop | `scoop info <pkg>` | `Version:` |

Failures fall into bounded `BackendError` variants (`NotFound`,
`ManagerMissing`, `Timeout`, `ParseFailed`, `PermissionDenied`,
`Other`). Every variant maps to a stable `error_kind` on the
`maintenance.check_failed` event so on-call dashboards don't have to
guess.

Per-backend wall-clock cap: 5 s hard timeout via
`tools::common::probe_with_timeout` (already exists for `diagnose`
k8s probes). A hung `brew` doesn't hang setup.

### Concurrency

`checker.rs` fans out per-tool via a bounded `rayon` scope,
`max(2, num_cpus / 2)` workers. Idempotent — every backend either
returns a version string or a bounded error. No shared mutable state
across workers except the report accumulator (`Mutex<Report>`; held
across map inserts only, never across shell-outs).

### Cache format

```json
{
  "schema_version": 1,
  "entries": {
    "<pkg-manager>:<pkg-id>": {
      "latest": "1.2.3",
      "checked_at_unix": 1721760000,
      "backend_error": null
    }
  }
}
```

TTL = 24 h from `checked_at_unix`. `backend_error` cached with a
shorter 1 h TTL so transient failures don't wedge the check for a
day. Cache read failure → treat as empty; cache write failure →
best-effort, emits `maintenance.cache_write_failed`.

### CLI surface

`jarvy check-updates` — new top-level command.

```
jarvy check-updates [--refresh] [--background] [--format json|human]
                    [--only <tool>[,<tool>...]] [--ignore <tool>[...]]
                    [--include-unchecked]
```

- No flags: read cache, print report from what's there. Fast, no
  network. Kicks off a background refresh if the cache is stale.
- `--refresh`: foreground refresh — blocks, streams progress to
  stderr, prints final report. Takes the cache lock.
- `--background`: internal / advanced. Refresh silently, write to
  cache, exit. This is what setup spawns and what a cron job would
  use. No stdout, minimal stderr, honors the lock.
- `--format` inherits the PRD-051 `Outputable` pattern.
- `--only` / `--ignore` narrow the tool set.
- `--include-unchecked` lists version-managed / custom-install tools
  in the report body (default: rolls them into a summary line).

Exit codes:

- `0` No updates found (or `--format json` succeeded).
- `1` Updates available (advisory, but scriptable).
- `2` Config error.
- `3` Backend unavailable (no supported package manager on PATH).

`jarvy setup` runs the check in `notify_on = "setup"` mode: cache-only
(never triggers a fresh network fan-out mid-setup), single-line
summary printed after the setup completion banner. `--quiet` and
`--format json` suppress the human line but still emit the telemetry.

### Setup phase wiring — background refresh, cache read

The setup path does **two** things and neither blocks:

1. **Read the cache** (`~/.jarvy/update-cache.json`). If entries are
   present and non-stale, format the summary line and print it after
   the setup completion banner. Cost: one file read + a semver
   compare per tool. No network. Sub-millisecond.
2. **Spawn the refresher.** After the summary is printed, fork a
   detached child (`jarvy check-updates --refresh --background`)
   with stdin/stdout/stderr redirected to `/dev/null` (or `NUL` on
   Windows) via `Command::spawn` — Jarvy's own process exits
   without waiting. The child writes cache entries as they land so
   partial results survive a mid-refresh crash. Next `jarvy setup`
   (or any command that reads the cache) sees the refreshed data.

This is a "second-run-onward" UX. First `setup` on a fresh box shows
no summary line (empty cache) and quietly kicks off the background
refresh so run #2 is populated. This is the intended tradeoff — the
alternative is a background daemon we've explicitly rejected.

**Spawn contract**:

- `posix_spawn` on Unix with `setsid()` (via `nix::unistd::setsid`
  or the `daemon(3)`-style double-fork if `setsid` is unavailable)
  so the child survives the terminal exit.
- `CreateProcess` with `DETACHED_PROCESS | CREATE_NO_WINDOW` on
  Windows.
- Refresher acquires an advisory lock at
  `~/.jarvy/update-cache.lock` — second invocation while a refresh
  is running exits silently (`maintenance.refresh_already_running`).
- Refresher writes its PID + `started_at` to
  `~/.jarvy/update-cache.pid`. If the file is stale (>1 h and PID
  not alive), the next spawn ignores it and takes the lock. Handles
  the "child crashed without cleanup" case without a supervisor.
- Refresher enforces a hard wall-clock cap of `5 * tool_count`
  seconds (min 60 s, max 600 s). Beyond the cap it writes what it
  has and exits — never wedges forever.

**Sandbox / CI / non-TTY**: the *spawn* is skipped in these modes
(`sandbox::is_sandbox()`, `ci::is_ci()`, `!isatty(stderr)`). Cache
*reads* still happen — CI can populate the cache once via
`jarvy check-updates` in a scheduled workflow if it wants the summary.

**Foreground refresh path**: `jarvy check-updates --refresh` (no
`--background`) is the interactive, blocking, foreground path. Used
by humans on demand and by the CI scheduled workflow. Streams
progress to stderr, prints results to stdout, exits when done. Same
lock semantics — refuses to run concurrently with a background
refresher (or takes over if the background one is stale).

Runs behind `observability::telemetry_gate::is_enabled()` for its
event trail same as every other phase.

### Telemetry (all gated, mirrors CLAUDE.md conventions)

| Event | Level | Fields |
|---|---|---|
| `maintenance.phase_started` | info | `mode = "cli_foreground" \| "cli_background" \| "setup_read"`, `tool_count`, `cache_ttl_hours` |
| `maintenance.phase_completed` | info | `mode`, `tools_checked`, `updates_available`, `unchecked`, `errors`, `duration_ms` |
| `maintenance.phase_skipped` | info | `reason = "disabled_by_config" \| "disabled_by_env" \| "sandbox" \| "ci" \| "non_tty" \| "no_provisioner_tools" \| "cache_fresh"` |
| `maintenance.background_spawned` | info | `pid`, `tool_count` — setup successfully forked a refresher |
| `maintenance.background_spawn_failed` | warn | `error_kind = "fork" \| "exec" \| "io"` — never fatal; setup continues |
| `maintenance.refresh_already_running` | debug | `held_by_pid`, `age_seconds` — lock held by another refresher |
| `maintenance.refresh_stale_lock_reclaimed` | warn | `stale_pid`, `age_seconds` — previous refresher crashed without cleanup |
| `maintenance.refresh_timeout` | warn | `deadline_secs`, `tools_completed`, `tools_pending` — hit the wall-clock cap |
| `maintenance.check_started` | debug | `tool`, `backend`, `pkg_id` |
| `maintenance.check_completed` | debug | `tool`, `backend`, `installed`, `latest`, `is_upgrade`, `from_cache`, `duration_ms` |
| `maintenance.check_failed` | warn | `tool`, `backend`, `error_kind = "not_found" \| "manager_missing" \| "timeout" \| "parse_failed" \| "permission_denied" \| "other"` |
| `maintenance.stale_tool` | info | `tool`, `backend`, `installed`, `latest`, `age_days` (only when parseable via semver + registry publish date if backend surfaces it; else omitted) |
| `maintenance.cache_hit` / `cache_miss` | debug | `entry_key`, `age_seconds` |
| `maintenance.cache_read_failed` | warn | `error_kind = "io" \| "json"` (path NOT logged) |
| `maintenance.cache_write_failed` | warn | `error_kind` |
| `maintenance.remote_refused` | warn | `reason = "allow_remote_not_set"` |
| `maintenance.backend_unavailable` | warn | `backend`, `os` (setup phase records this once per backend per run, not per tool) |

Metric: `jarvy.maintenance.stale_tools` (gauge, tagged
`{backend, project}`). Enables the "how far behind is my fleet?"
dashboard without any org needing to run its own scraper.

### Version-manager and custom-install tools

Rolled up into an `unchecked` bucket:

```
2 tools skipped (managed by version manager):
  - rust (rustup)
  - node (nvm)
```

`--include-unchecked` breaks them out per-tool for scripting. Never
emitted as `stale_tool`.

### Human output shape

```
Freshness advisory (12 tools checked, 3 with newer versions):

  jq           1.7.1  → 1.8.0    (brew)
  cargo-nextest 0.9.72 → 0.9.85   (cargo)
  kubectl      1.30.2 → 1.31.4   (brew)

Run `jarvy check-updates --format json` for machine-readable output.
Configure via [maintenance] in jarvy.toml.
```

Colours honor the existing `NO_COLOR` / non-TTY conventions in
`observability`.

### JSON output shape

```json
{
  "schema_version": 1,
  "generated_at": "2026-07-23T15:48:00Z",
  "summary": {
    "tools_checked": 12,
    "updates_available": 3,
    "unchecked": 2,
    "errors": 0
  },
  "updates": [
    {
      "tool": "jq",
      "backend": "brew",
      "installed": "1.7.1",
      "latest": "1.8.0",
      "direction": "upgrade"
    }
  ],
  "unchecked": [
    { "tool": "rust", "reason": "version_manager", "manager": "rustup" }
  ],
  "errors": []
}
```

Matches the shape of `drift check --format json` so downstream
tooling can join the two.

## Testing

- Unit tests per backend parser using captured fixture output
  (mirrors the `tools::version::extract_version` test harness).
- Integration test (`tests/maintenance_cli.rs`) with `JARVY_FAST_TEST`
  stubbing every backend to a fixed version string, asserting
  human + JSON output, exit codes, cache round-trip, `--only` /
  `--ignore` filters, TTL expiry.
- Trust-boundary tests: remote config `check_updates = true` refused
  without `allow_remote`, sandbox / CI auto-skip, env kill-switch.
- Cache tests: corrupt JSON → treated as miss; stale entry → refresh;
  write failure → best-effort, no user-visible error.
- Telemetry gate test: `telemetry.enabled = false` → zero
  `maintenance.*` events on the OTLP sink.

## Open questions

- **First-run empty summary UX.** First `jarvy setup` on a fresh
  box shows no summary line because the cache is empty. Options:
  (a) print "checking upstream versions in the background — results
  will appear on your next `jarvy` invocation" so users understand
  why run #1 is silent and run #2 isn't; (b) print nothing (cleaner
  first-run banner, invisible feature until run #2). Leaning (a).
- **Refresher detachment on macOS.** `setsid()` + `posix_spawn` is
  well-trodden but some sandboxed macOS environments (App
  Sandbox / Endpoint Security) refuse to detach. The refresher
  should degrade gracefully — if detachment fails, log
  `maintenance.background_spawn_failed { error_kind = "detach" }`
  and skip. Setup completes normally either way.
- **Rate limits.** `brew info --json=v2` hits the Homebrew API for
  formulae that aren't in the local tap cache. High-tool-count
  projects (30+) may burn Homebrew's per-IP budget. Mitigation:
  respect the shared TTL cache aggressively, back off on HTTP 429
  surfaced by brew stderr, add `maintenance.backend_rate_limited`
  if it becomes a real problem.
- **Windows scoop / choco / winget parity.** `winget show` output
  format has changed between Windows releases; may need a version
  probe on first use. Choco's `--limit-output` is stable but choco
  itself is less common. Ship winget as the Windows primary and
  add choco / scoop as follow-up backends.
- **Publish-date age.** `stale_tool.age_days` requires a publish
  timestamp. Brew surfaces it; cargo/npm require an extra registry
  call. Ship with `age_days` optional; add cargo/npm date probes
  behind `--include-age` if maintainers ask for it.
- **Interaction with `jarvy update`.** `jarvy update` self-updates
  the Jarvy binary; `jarvy check-updates` inspects provisioner
  tools. Two different domains, but the name collision will
  confuse users. Consider aliasing (`jarvy tools update-check`?) or
  documenting the split prominently. Prefer keeping the top-level
  `check-updates` verb; solve confusion via CLI help copy.

## Verification

- Implementation plan file to be linked here on kickoff (per
  PRD-056 convention).
- Smoke matrix: macOS (brew + cargo + npm), Ubuntu (apt + cargo +
  npm), Windows (winget + cargo + npm). Each platform runs the CLI
  + setup integration with real backends against a fixture project
  before merge.
