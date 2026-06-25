# CLAUDE.md

@SKILL.md

Guidance for Claude Code working in this repo. Per-module deep dives live in `src/<module>/` and `docs/`; this file captures only what can't be derived by reading the code.

## Build Commands

```bash
cargo build                                       # Debug
cargo build --release                             # Release
cargo fmt --all                                   # Format
cargo clippy --all-features -- -D warnings        # Lint (CI gate)
cargo check --verbose                             # Type check
cargo test --verbose -- --show-output             # All tests
cargo test --test cli_dispatch -- --show-output   # Single integration test
cargo run -p cargo-jarvy -- new-tool <name>       # Scaffold tool
```

## Architecture

Cross-platform CLI that provisions dev environments from `jarvy.toml` using native package managers (brew on macOS, apt/dnf/etc on Linux, winget/choco/scoop on Windows).

### Module Map

- `src/main.rs` — minimal entry (~540 lines): args, telemetry init, dispatch
- `src/cli/` — `args.rs` (`Cli`, `Commands`, `OutputFormat`), `subcommands.rs` (nested enums)
- `src/commands/` — one file per handler (`setup_cmd.rs`, `tools_cmd.rs`, `roles_cmd.rs`, `ai_hooks_cmd.rs`, `mcp_register_cmd.rs`, `drift_cmd.rs`, `logs_cmd.rs`, `ticket_cmd.rs`, …)
- `src/config.rs` — `jarvy.toml` parser. **`TOP_LEVEL_SECTIONS` const is canonical**; `validate.rs` consumes it. Compile-time destructure test `config::tests::top_level_sections_matches_config_fields` rejects adding a `Config` field without updating both.
- `src/tools/` — `registry.rs` (global `OnceLock<RwLock<HashMap>>`), `common.rs` (`Os`, `InstallError`, `run`, `has`, `cmd_satisfies`), `spec.rs` (`ToolSpec` + `define_tool!` macro)
- `src/packages/` — npm / pip / cargo / nuget handlers
- `src/roles/` — role-based config with inheritance (PRD-033)
- `src/ai_hooks/` — guardrail hooks distributed to Claude Code, Cursor, Codex, Windsurf, Cline, Continue. See `docs/ai-hooks.md`.
- `src/mcp_register/` — auto-registers Jarvy MCP server with the same 6 agents. See `docs/mcp-registration.md`.
- `src/mcp/extended_tools.rs` — broader Jarvy surface exposed over MCP (drift / roles / templates / services). Mutating tools gated by `gate_mutation` + `MutationCtx` (rate limit → stderr TTY confirm → audit log). Workspace containment via `safety::resolve_within_workspace` (canonical-root check, refuses `..`, absolute escapes, endpoint symlinks). Workspace root = `JARVY_MCP_WORKSPACE` env or cwd.
- `src/drift/` — state baseline + version policy (Major/Minor/Patch/Exact) + JSON/human reporter
- `src/network/` — proxy + CA bundle resolution. Priority: env > tool override > global. Propagates `HTTP(S)_PROXY`, `NO_PROXY`, `CURL_CA_BUNDLE`, `SSL_CERT_FILE`, `NODE_EXTRA_CA_CERTS`, `GIT_SSL_CAINFO`.
- `src/git/` — git config automation (identity, signing, aliases). `ConfigValue` = plain string or `{ env, default }`. Signing format auto-detected (`.pub` → ssh, else gpg). Credential helper defaults per-OS.
- `src/update/` — self-updater. Detects install method (brew/cargo/apt/dnf/pacman/winget/choco/scoop/binary). Env: `JARVY_UPDATE`, `JARVY_UPDATE_CHANNEL`, `JARVY_PINNED_VERSION`. CI auto-disables.
- `src/observability/` — tracing-subscriber wiring, sanitizer (`Cow<'_, str>` no-alloc on no-match), rolling appender (`tracing_appender::rolling` daily), `non_blocking` writer with `WorkerGuard` held in `analytics::FILE_LOGGER_GUARD`. `shutdown_logging()` flushes `SdkLoggerProvider` + worker guard before exit.
- `src/logging/` — thin re-export over `observability/` + log-file helpers (`read_recent_logs`, `get_log_stats`, `clean_logs`)
- `src/ticket/` — debug-bundle ZIP for support
- `src/remote.rs` — remote config fetch + caching (`fetch_remote_config`, `transform_github_url`)
- `src/interactive.rs` — menu when run without subcommand

### Tool Implementation (`define_tool!` macro)

Each tool lives in `src/tools/{name}/{mod.rs, {name}.rs}`. Minimal example:

```rust
define_tool!(JQ, {
    command: "jq",
    macos: { brew: "jq" },
    linux: { uniform: "jq" },
    windows: { winget: "jqlang.jq" },
});
```

Macro slots: `macos.brew` / `macos.cask`, `linux.uniform` OR `linux.{apt,dnf,pacman,apk}`, `windows.winget` / `windows.choco`, `custom_install: Some(fn)` for shell-script installs (nvm/rustup/brew), `default_hook: { description, script }`, `depends_on: &[...]` (strict — all required), `depends_on_one_of: &[...]` (flexible — one required), `category: "messaging" | …`. Register in `src/tools/mod.rs::register_all()`.

**Non-obvious rules:**

- **Omit unsupported platforms.** No first-party `winget` / `choco` package on Windows? OMIT the block. Placeholder ids (e.g. `Pivotal.RabbitMQ`) create supply-chain exposure — anyone claiming the publisher namespace can ship a malicious installer that `winget install -e --id` pins to. Runtime emits `tool.unsupported` (routed via `InstallError::is_no_platform_installer()`) — that's the UX. Add a one-line `// No first-party winget manifest as of YYYY-MM; install from <upstream-url>.` per the `kaf`/`kafkactl`/`emqx`/`kn` examples.
- **Dash ↔ underscore aliasing.** `define_tool!(NATS_SERVER, …)` stringifies as `"nats_server"`. `registry::get_tool()` falls back to a `-` ↔ `_` swap so `nats-server = "latest"` also resolves. `validate::validate_tools` mirrors it. Pin whichever form upstream docs use; users get the other free.
- **Brew tap auto-tap.** When `macos.brew` (or `linux.brew` fallback) is `org/tap/formula` (exactly two slashes), install path runs `brew tap org/tap` first so a fresh box doesn't surface an "untrusted tap" error. Soft-fail; already-added tap doesn't block.
- **Categories.** Set `category: "messaging" | "workflow" | …` so `tool.installed` carries the label. Operators can graph "what fraction of NATS rollouts succeeded?" without pivoting on tool name. Introduce new category once 3+ tools qualify; backfill opportunistically.
- **Default hooks** are idempotent (scripts check before mutating), advisory (failures = warnings), overridable (user `[hooks.tool]` wins).
- **Dependencies** affect install order via topo sort. Strict missing = warn + install anyway. Flexible: if one in config, install first; if one already present, satisfied; else advisory.

### Trust Boundaries (cross-cutting)

Remote-fetched configs (loaded via `jarvy setup --from <url>`) are tagged `ConfigOrigin::Remote` (see `Config::mark_remote()`). They may **NARROW** trust but cannot **BROADEN** it:

- `[ai_hooks] allow_custom_commands = true` — refused for remote configs; library hooks always allowed.
- `[mcp_register] allow_custom_servers = true` — refused for remote configs; built-in `jarvy` server always allowed.
- `[packages] allow_remote = true` — without this, remote configs cannot install `[npm]/[pip]/[cargo]/[nuget]` entries.
- Project-config telemetry overrides (endpoint) — refused with stderr warning. Endpoint override only via `JARVY_OTLP_ENDPOINT`.

Package-name validation (`validate_package_name` / `validate_package_version`) refuses leading-`-`, URL schemes, shell-meta, and control bytes (ESC/BEL/DEL/NUL — closes ANSI injection in dry-run preview). `jarvy validate` runs them on every `[npm]/[pip]/[cargo]/[nuget]` entry.

### Config Files

- `jarvy.toml` (project) — tools, packages, roles, hooks, etc.
- `~/.jarvy/config.toml` (global) — telemetry, update, machine fingerprint
- `.jarvy/state.json` (project) — drift baseline

### Logs & Tickets

- Logs: `~/.jarvy/logs/jarvy.log` (+ `.1.gz`, …). Daily rotation, `[logging]` section in `jarvy.toml` controls level/format/retention.
- Tickets: `~/.jarvy/tickets/JARVY-YYYYMMDD-xxxxxxxx.zip`. `jarvy ticket {create,show,list,clean}`.

## Telemetry

OTEL-based, **opt-out by default**. Config in `~/.jarvy/config.toml::[telemetry]` (`enabled`, `endpoint`, `protocol`, `logs`, `metrics`, `traces`, `sample_rate`). Env overrides: `JARVY_TELEMETRY` (`0` disables, `1` enables), `JARVY_OTLP_ENDPOINT`, `JARVY_OTLP_PROTOCOL`, `JARVY_OTLP_{LOGS,METRICS,TRACES}`, `JARVY_OTLP_SAMPLE_RATE`. CI / unattended sandboxes auto-disable unless `JARVY_TELEMETRY=1`. CLI: `jarvy telemetry {status,enable,disable,set-endpoint,test,preview}`. Module: `src/telemetry.rs`.

### Event Taxonomy — stable contract for AI agents and log queries

| Event | Source | Notes |
|-------|--------|-------|
| `tool.requested` | per tool in config | Setup phase |
| `tool.installed` | per successful install | Setup phase |
| `tool.failed` | per failed install | Setup phase |
| `tool.unsupported` | setup unknown-tool loop + `--request` | Uniform field shape across both call sites |
| `tool.already_installed` | skip-detection path | `install_path`, `detection_method`, `prompted_user` |
| `setup.started` / `setup.completed` | run lifecycle | Carries duration, counts |
| `hook.started` / `hook.completed` / `hook.failed` / `hook.timeout` | per hook | |
| `packages.phase_started` / `packages.phase_completed` | run_packages_phase | Carries `dry_run`, `npm`, `pip`, `cargo`, `nuget` booleans, `duration_ms` |
| `packages.phase_skipped` | early-return | `reason`, `dry_run` |
| `packages.phase_previewed` | dry-run preview | Per-ecosystem `*_count` (renamed from `packages.dry_run`) |
| `packages.remote_refused` | trust gate refusal | `reason` (e.g. `allow_remote_packages_not_set`) |
| `packages.install_failed` | install_packages outer error | One per ecosystem on failure |
| `package.requested` | per-package entry | `ecosystem`, `package`, `version`, `platform` |
| `package.installed` | per-package success | + `duration_ms` |
| `package.failed` | per-package failure | `error!` level; `error` redacted |
| `package_command.failed` | run_package_command non-zero | + `stderr_tail` (last 4KB) |
| `commands.extras_refused_keys` | interactive menu key sanitizer | `count` (control-byte / Trojan-Source keys dropped) |
| `ai_hook.phase_started` / `ai_hook.phase_completed` | apply phase | `agents`, `applied`, `refused_local`, `refused_remote`, `failures`, `duration_ms` |
| `ai_hook.agent_applied` / `ai_hook.agent_failed` | per agent | `agent_failed` carries `error_type` only — formatted message NOT emitted (no user-content leak) |
| `ai_hook.provisioned` | per hook | `agent`, `hook_name`, `library_source` |
| `ai_hook.custom_refused_summary` | trust gate | `local_count`, `remote_count` — counts only, no names/bodies |
| `ai_hook.check_completed` | drift check | `agents_checked`, `drifted_agents` |
| `ai_hook.windows_auto_translated` | Bash→PowerShell xlat | `agent`, `hook_name` |
| `mcp_register.phase_started` / `mcp_register.phase_completed` | apply phase | `agents`, `applied`, `refused_local`, `refused_remote`, `failures`, `duration_ms` |
| `mcp_register.agent_applied` / `mcp_register.agent_failed` | per agent | `agent_failed`: `error_type` only |
| `registry.sync.started` / `registry.sync.completed` | `jarvy registry sync` lifecycle | `registry_url` (redacted), `require_signature`, `tools_synced`, `tools_removed`, `signature_verified`, `duration_ms` |
| `registry.sync.failed` | preflight + per-stage error returns | `stage = "preflight" \| "manifest_parse" \| ...`, `reason` (when preflight), `error` |
| `registry.sync.signature_refused` | cosign verification rejected | `registry_url`, `identity_regexp`, `oidc_issuer`, `reason` |
| `registry.signature_disabled` | `require_signature = false` escape hatch | `registry_url` |
| `registry.sync.sha_mismatch` | per-tool sha verification fails | `tool`, `worker_id`, `url` (redacted), `expected`, `actual` |
| `registry.sync.tool.start` / `registry.sync.tool.synced` | per-tool in parallel loop (debug) | `tool`, `worker_id`, `bytes` |
| `registry.sync.tool_fetch_failed` / `tool_parse_failed` / `tool_write_failed` | per-tool failures inside the parallel loop | `tool`, `worker_id`, plus `url`/`error` |
| `registry.fetch.start` / `registry.fetch.completed` | per HTTPS GET in `fetch_bounded` (debug) | `url`, `max_bytes`, `bytes` |
| `registry.fetch.failed` | non-200 / size cap / network err | `url`, `registry_url`, `error` |
| `registry.cache.swap_failed` | atomic-swap of `tools.new/` into `tools/` failed | `stage = "promote" \| "rollback"`, `error` |
| `registry.cache.index_built` | parsed-tools index written | `accepted_count` |
| `registry.cache.index_build_failed` | index write IO failure (non-fatal) | `error` |
| `registry.cache.index_perms_unsafe` | `index.json` mode looser than `0700` after chmod | `mode` |
| `registry.cache.index_hit` / `registry.cache.index_miss` | plugin loader read the index cache (debug) | `tools_count`, `synced_at_unix` / `reason` |
| `registry.cli.sync_failed` | `jarvy registry sync` exit-code mapping | bounded `error_kind` label only |
| `signature.skipped` / `signature.verified` / `signature.failed` | cosign verify path (was `update.signature.*` — renamed because shared with registry) | `file`, plus `reason` (skipped) or `error` (failed) |

**`tool.unsupported` fields** (uniform across setup and `--request`):
```
tool, version?, source, platform, suggestions, channel,
fallback_issue_url, scaffold_cmd, exit_code, opt_in_bypassed
```
- `source`: `config` | `mcp` | `cli` | `request`
- `channel`: `telemetry` (canonical) | `manual` (telemetry disabled — user must use `fallback_issue_url`)
- `opt_in_bypassed`: `true` only on `--request` (user typed command = implicit consent; OTEL counter fires regardless of global opt-in). `--request` also emits `counter_fired`; when `false`, falls back to `manual` channel.
- `fallback_issue_url`: present only when `channel = "manual"`; telemetry-on path omits to keep log lines short.
- `exit_code`: always `8` (TOOL_UNSUPPORTED). Setup emits per unknown tool but process exits `8` only when ALL configured tools unknown; mixed runs return `0`.
- **Metric counter**: `jarvy.tool.unsupported` (renamed from `jarvy.tool.not_supported` to match event name).

**Telemetry gate.** Every `package.*` / `packages.*` / `package_command.failed` event reads `observability::telemetry_gate::is_enabled()` before emitting — populated by `telemetry::init` at startup. Without it, prior implementation leaked package events to OTLP when `telemetry.enabled = false` but an endpoint was set for unrelated reasons. Broke documented opt-in contract.

## Testing

Integration tests in `/tests/`. Env vars:
- `JARVY_TEST_MODE=1` — disables interactive prompts
- `JARVY_FAST_TEST` — skips external command execution

## Exit Codes

- `0` Success
- `2` CONFIG_ERROR (malformed `jarvy.toml`)
- `3` PREREQ_MISSING (package manager not found)
- `4` NETWORK_TIMEOUT (network / proxy failure)
- `5` PERMISSION_REQUIRED (sudo needed)
- `6` INCOMPATIBLE_OS_ARCH
- `7` HOOK_FAILED (pre_setup / post_install / post_setup)
- `8` TOOL_UNSUPPORTED (every configured tool unknown; mixed runs return `0`)

Drift command additionally: `1` drift detected, `2` no baseline.

## Conventions

- Rust 2024 edition
- Conventional Commits: `feat:`, `fix:`, `docs:`, `chore:`, `refactor:`, `test:`
- Prefer stdlib + existing deps over new crates
- `cargo fmt` + `cargo clippy` before committing
