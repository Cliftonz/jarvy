# CLAUDE.md

@SKILL.md

Guidance for Claude Code working in this repo. Per-module deep dives live in `src/<module>/` and `docs/`; this file captures only what can't be derived by reading the code.

## Model Routing & Orchestration (Fable 5 workflow)

Fable 5 drives this repo. It is expensive — delegate token-heavy grunt work to
cheaper models via the shell-out skills below; keep taste- and judgment-heavy
work on Fable/Opus. Reasoning effort stays **high** — never x-high/max/ultracode
for routine work (it overthinks, overbuilds, and costs multiples for marginal gain).

**Glossary** (evaluation axes used throughout):
- **Intelligence** — how hard a problem the model handles unsupervised
- **Taste** — UI/UX, code quality, API design, copy
- **Cost** — effective cost given active subscriptions (Claude Code + Codex CLI)

**Routing table** (1–10; cost 10 = cheapest):

| Model | Cost | Intelligence | Taste | Default use |
|---|---|---|---|---|
| GPT via `codex exec` | 9 | 8 | 4 | Bulk mechanical work: clear-spec implementations, migrations, log digging, big-file reading, independent second-opinion reviews |
| Sonnet 5 | 5 | 6 | 5 | Sub-agent fan-out; thin proxy wrapper around external CLIs inside workflows (low effort) |
| Opus 4.8 | 6 | 8 | 8 | Reviews and plans when Fable budget matters |
| Fable 5 | 3 | 10 | 10 | Orchestration, architecture, API design, final review, anything user-facing |
| Haiku 4.5 | — | skip | — | Not for real work |

**Routing rules:**
- These are defaults, not limits. If a cheaper model's output misses the bar,
  rerun with a smarter model **without asking**. Judge the output, not the price tag.
- Use cheap models to gather information and try things before moving work to
  an expensive one; never let cost pick the wrong model for the final pass.
- Bulk/mechanical → codex (see `external-review` / `external-implementation`
  skills). Taste- or user-facing → Fable or Opus.
- Reviews of plans and implementations: Fable/Opus primary; codex as an extra
  independent perspective.
- Prompting codex: keep prompts much simpler than for Claude — one or two
  sentences of task + focus. Don't prompt it like it's Claude.

**Orchestration rules:**
- **Subagents do NOT compile, test, or commit. The orchestrator does.**
  Concurrent `cargo build`/`cargo test` across N worktrees saturates the
  CPU — 5 parallel builds means ~62 rustc processes fighting for cores, so
  a 3-minute test run takes 30+ and nothing finishes. A spawned
  implementation agent's job is to **write the change and report what it
  wrote** (files touched, intent, anything uncertain). It must not run
  `cargo build`/`clippy`/`test` or `git commit`/`push`. The orchestrator
  gathers all agents' findings, then serially — one cargo invocation at a
  time — compiles, tests, and commits each branch. If an agent needs a
  quick `cargo check` to validate a tricky change, that is the exception,
  not the default, and never in parallel with sibling agents.
- Don't pre-define agent archetypes — invent the right reviewers/implementers
  per task; every review has different needs.
- Workflows can only spawn Claude models. To use codex inside a workflow,
  spawn Sonnet at low effort as a thin wrapper that shells out to the codex
  CLI and reports back; label those sub-agents with a `codex-proxy:` prefix.
- Checkpoint-driven sequences (CI must pass → review → merge → rebase next)
  are orchestrated from the main session with worktrees — not one giant
  workflow, which barrels past checkpoints or stalls. Use workflows for what
  they're strong at: multi-agent review fan-out before a merge.
- Time-to-complete is an architecture signal on ad-hoc fixes: <3 min = low-risk;
  ~15 min = look closer before merging; 1 h+ = the architecture in that area is
  the problem — go deeper, don't merge blind.

## Build Commands

```bash
cargo build                                              # Debug
cargo build --release                                    # Release (ships WITHOUT test-bypass — bypass env vars are inert)
cargo fmt --all                                          # Format
cargo clippy --all-features -- -D warnings               # Lint (CI gate)
cargo check --verbose                                    # Type check
cargo test --all-features --verbose -- --show-output     # All tests (CI uses --all-features; equivalent to --features test-bypass)
cargo test --features test-bypass                        # Local-dev shorthand for the test-bypass-gated integration tests
cargo test --test cli_dispatch -- --show-output          # Single integration test
cargo run -p cargo-jarvy -- new-tool <name>              # Scaffold tool
```

**`test-bypass` feature.** Several integration tests use loopback HTTP
(`JARVY_LIBRARY_ALLOW_INSECURE_FETCH`, `JARVY_REGISTRY_ALLOW_INSECURE_FETCH`)
or a redirected home (`JARVY_TEST_HOME`). Those escape hatches are
compiled out of release builds (review item 15) — the env vars are
inert in shipped binaries. `cargo test` without `--features test-bypass`
silently skips the dependent integration tests (via `required-features`
in `Cargo.toml`); CI invocations pass `--all-features` so the gate is
automatic.

## Architecture

Cross-platform CLI that provisions dev environments from `jarvy.toml` using native package managers (brew on macOS, apt/dnf/etc on Linux, winget/choco/scoop on Windows).

### Module Map

- `src/main.rs` — minimal entry (~270 lines): args, telemetry init, OTLP flush. Dispatch + per-command glue live in `commands/dispatch.rs` (PRD-037).
- `src/cli/` — `args.rs` (`Cli`, `Commands`, `OutputFormat`), `subcommands.rs` (nested enums)
- `src/commands/` — one file per handler (`setup_cmd.rs`, `tools_cmd.rs`, `roles_cmd.rs`, `ai_hooks_cmd.rs`, `mcp_register_cmd.rs`, `drift_cmd.rs`, `logs_cmd.rs`, `ticket_cmd.rs`, `run_cmd.rs`, …) plus `dispatch.rs` (CLI routing table). `run_cmd.rs` = `jarvy run [name] [-- args]`, the npm-run-style runner for `[commands]`: explicit name = consent (chaining metachars allowed, command printed ANSI-stripped before exec, child exit code propagated, cwd = the config file's directory) — deliberately looser than the interactive menu's `classify_shell_command` gauntlet, NO implicit `cargo run`/`cargo test` fallback; NUL and (Windows) `%`-bearing `--` args refused. npm-style lifecycle hooks via `resolve_hook`: `pre<name>`/`post<name>` AND the colon spelling `"pre:<name>"`/`"post:<name>"` (colon wins when both defined, note printed); failing pre aborts the main, post runs only after success, `--` args reach the main command only — each hook executes through `execute_one` with its own `run.command.*` label. `jarvy shell-init` snippet defines `jr` as shorthand; `jarvy shell-init --apply` writes the loader line into the shell rc idempotently (nushell gets a materialized `~/.jarvy/init.nu`). `commands/shared.rs` holds the mechanics both surfaces share (`sanitize_for_display`, `spawn_shell` incl. Windows `cmd /C`, `quote_shell_arg`, `short_cmd_hash`); the `[commands]` loader + Trojan-Source key sanitizer live in `config.rs::read_commands_config` (policy stays per-caller: menu defaults on error, `jarvy run` hard-errors).
- `src/config.rs` — `jarvy.toml` parser. **`TOP_LEVEL_SECTIONS` const is canonical**; `validate.rs` consumes it. Compile-time destructure test `config::tests::top_level_sections_matches_config_fields` rejects adding a `Config` field without updating both.
- `src/tools/` — `registry.rs` (global `OnceLock<RwLock<HashMap>>`), `common.rs` (`Os`, `InstallError`, `run`, `has`, `cmd_satisfies`), `spec.rs` (`ToolSpec` + `define_tool!` macro)
- `src/packages/` — npm / pip / cargo / nuget / gem / go handlers
- `src/roles/` — role-based config with inheritance (PRD-033)
- `src/agents.rs` — canonical `Agent` enum shared across `ai_hooks`, `mcp_register`, and `skills`. The three former per-subsystem enums (`AgentTarget`, `McpAgentTarget`, `SkillAgent`) are now `pub use crate::agents::Agent as <Name>` aliases so call sites compile unchanged; a new agent variant lands in all three subsystems atomically.
- `src/ai_hooks/` — guardrail hooks distributed to Claude Code, Cursor, Codex, Windsurf, Cline, Continue. See `docs/ai-hooks.md`.
- `src/mcp_register/` — auto-registers Jarvy MCP server with the same 6 agents. See `docs/mcp-registration.md`.
- `src/mcp/extended_tools.rs` — broader Jarvy surface exposed over MCP (drift / roles / templates / services). Mutating tools gated by `gate_mutation` + `MutationCtx` (rate limit → stderr TTY confirm → audit log). Workspace containment via `safety::resolve_within_workspace` (canonical-root check, refuses `..`, absolute escapes, endpoint symlinks). Workspace root = `JARVY_MCP_WORKSPACE` env or cwd.
- `src/drift/` — state baseline + version policy (Major/Minor/Patch/Exact) + JSON/human reporter
- `src/network/` — proxy + CA bundle resolution. Priority: env > tool override > global. Propagates `HTTP(S)_PROXY`, `NO_PROXY`, `CURL_CA_BUNDLE`, `SSL_CERT_FILE`, `NODE_EXTRA_CA_CERTS`, `GIT_SSL_CAINFO`.
- `src/net/` — shared HTTP primitives. `agent` (ureq config + UA), `bounded_fetch` (HTTPS-only refusal, bounded read, loopback-bypass parser — used by both `library_registry::fetch` and `registry_remote::fetch`; each consumer passes its own env-var name so test isolation stays per-subsystem), `url_encode`.
- `src/git/` — git config automation (identity, signing, aliases). `ConfigValue` = plain string or `{ env, default }`. Signing format auto-detected (`.pub` → ssh, else gpg). Credential helper defaults per-OS. `[git.extra]` is a free-form escape hatch (`HashMap<key,value>`) for un-modeled keys, applied last so they override typed fields. **Remote-config trust gate** (`run_git_phase`): a `ConfigOrigin::Remote` config cannot apply `[git]` unless `[git] allow_remote = true`, and even then writes are forced to `--local` scope (mirrors `[git_hooks] allow_remote`; without it a remote could write `~/.gitconfig`). `[git.extra]` gauntlet is `check_extra_entry` (shared by `configure_extra` write-path AND `extra_write_plan`, which the dry-run preview consumes so preview == apply): `validate_extra_key` (dotted grammar + flag-injection, ≤256B) → leading-`-` value refusal (argv option-injection) → `is_exec_capable_key` refuses keys whose value git *executes* (`core.pager`/`sshCommand`/`hooksPath`/`askPass`/`fsmonitor`(non-bool)/`pager.*`/`merge.*.driver`/`remote.*.uploadpack`/`init.templateDir`/`filter.*.clean`/`*.textconv`/… — RCE the `!` filter misses) unless `JARVY_ALLOW_GIT_EXEC_KEYS=1` (`exec_key_value_is_safe` exempts `core.fsmonitor=true|false`) → `check_not_protect_downgrade` (protectNTFS/HFS=false, `safe.directory=*`/`safe.bareRepository=all`, `fsck.*`/`fetch.fsck.*`/`receive.fsck.*=ignore`, `*.fsckObjects=false`, `*.sslVerify=false`; `JARVY_ALLOW_GIT_PROTECT_DOWNGRADE=1` override, both refusal + override emit events) → `value_is_shell_escape` (`!` after trim). **Typed `editor`/`credential_helper` fields** funnel through `set_config`, which guards the shell-interpreted keys by VALUE (`value_has_shell_metachars` / `credential_helper_is_program_path`) — a bare command + flags is allowed, injection/path is refused (the RCE the `[git.extra]` denylist doesn't reach). `os_defaults` (default on; `= false` opts out) applies `os_default_plan()` (pure) for unset keys — autocrlf/longpaths(Win)/precomposeunicode(mac) + recommended `fetch.prune`/`rerere.enabled`/`merge.conflictStyle=zdiff3`; `os_defaults_to_write` + `parse_null_config` skip already-matching keys (one `existing_config()` read). All env opt-ins parse via `env_flag_enabled`; ALL `git_config.*` events go through the `gated_*!` macros. See `docs/git-config.md`.
- `src/git_hooks/` — pre-commit framework integration (PRD-048). `[git_hooks]` block in `jarvy.toml`. Auto-install during `jarvy setup` between git-config and ai-hooks phases. Husky / lefthook detected but handlers stubbed (`UnsupportedFramework` error). Remote-config trust gate via `[git_hooks] allow_remote`.
- `src/library_registry/` — shared library-registry pattern (PRD-054) reused by `[ai_hooks] library_sources`, `[mcp_register] library_sources`, `[skills] library_sources`. Three URL schemes: `https://...` (manifest fetch), `git+https://...@<ref>[#<subpath>]` (PRD-055 git clone, skills-only), `github:owner/repo@<ref>` (shorthand). One manifest format (`{schema_version, publisher, items: [{kind: ai_hook|mcp_server|skill, ...}]}`), HTTPS-bounded fetch, on-disk cache at `~/.jarvy/library.d/<sha256-of-url>/` (+ `git/` subtree for git sources), in-process resolver. Companion artifacts (hook `bash_url`/`powershell_url` bodies, skill `companion_files`) fetch via `companion::fetch_verified` — mandatory manifest sha pin, content-addressed cache at `~/.jarvy/library.d/companions/<sha256>`, no unverified fetch path. Remote-fetched configs CANNOT declare `library_sources` (`library_registry::check_origin` refusal). Cosign sig verify scaffolded but not enforced in v1.
- `src/skills/` — AI agent skill installation (PRD-049 v1 + phase 2). `[skills]` block in `jarvy.toml` + `jarvy skills {install,update,remove,list,status,agents}` CLI. Pulls `SKILL.md` from a library_sources manifest, sha-verifies, writes to `~/.{agent}/skills/<name>/SKILL.md` (claude-code, cursor, codex, windsurf, cline, continue). Per-skill agent narrowing + publisher `supported_agents` filter. `.jarvy-skill.json` sidecar for drift detection. Phase 2: `update` compares sidecar version/sha against the library and reinstalls only on divergence (pinned mismatch refuses); `remove` deletes SKILL.md + sidecar idempotently (user companion files survive); `install <name>` resolves ad-hoc from library_sources at `latest` when the name isn't in `[skills.install]`. Skill names are validated against path traversal (`validate_skill_name`). Still open: skills.sh API, project-scope skills, version-range pinning.
- `src/progress.rs` — spinner abstraction over `indicatif` (PRD-052). Auto-disables on non-TTY, `--quiet`, `--format json`, sandbox / CI, `JARVY_NO_PROGRESS=1`. Used today by `jarvy update check` and `jarvy hooks {install,update}`; further integration follow-up.
- `src/discover/` — project tool auto-discovery (PRD-044 MVP). `jarvy discover [--apply] [--missing] [--format json]` scans the project root for marker files (Cargo.toml, package.json, go.mod, Dockerfile, k8s/, *.tf, Makefile, Justfile, …), infers versions from `rust-toolchain.toml` / `.nvmrc` / `.python-version` / `go.mod`, and either prints suggestions or merges them into `jarvy.toml`. Suggestions are validated against `tools::registry::registered_tool_names()` so we never recommend a tool jarvy can't install. Built-in rules only in v1 (custom rules file deferred).
- `src/workspace.rs` + `src/commands/workspace_cmd.rs` — monorepo support (PRD-047). `[workspace] members = [...]` declared in root `jarvy.toml`; per-member jarvy.toml inherits via `merge_configs` (provisioner table merges tool-by-tool with member winning). `jarvy workspace {list,show,validate}` is the read-only CLI surface — workspace-aware `jarvy setup --project <name>` orchestration is deferred. Empty `inherit = []` is treated as `["provisioner"]` for the show/list output so the common case "just works".
- `src/update/` — self-updater. Detects install method (brew/cargo/apt/dnf/pacman/winget/choco/scoop/binary). Env: `JARVY_UPDATE`, `JARVY_UPDATE_CHANNEL`, `JARVY_PINNED_VERSION`. CI auto-disables.
- `src/observability/` — log-config types (`LogConfig`/`LogLevel`/`LogFormat`), the `jarvy setup --profile` phase `Profiler`, sanitizer (`Cow<'_, str>` no-alloc on no-match), and `telemetry_gate`. The actual tracing-subscriber wiring (console/file/OTLP layers, CLI log-flag overrides) lives in `src/analytics.rs::init_logging`. `jarvy setup -q/-v/-vv/-vvv/--log-format/--log-file/--debug-filter/--profile` are wired through `main.rs` → `commands/dispatch.rs`: CLI verbosity filters the **console** layer only (not the registry `EnvFilter`, so `-q` doesn't starve `jarvy.log`/OTLP); `--debug-filter`/`-v` widen the registry filter and beat `RUST_LOG`. Rolling appender (`tracing_appender::rolling` daily) + `non_blocking` writers with `WorkerGuard`s held in `analytics::{FILE_LOGGER_GUARD, EXTRA_LOG_GUARD}`; `shutdown_logging()` flushes `SdkLoggerProvider` + both guards before exit. (The former `bundle`/`network_trace` modules were deleted — `DiagnosticBundle` duplicated `src/ticket/`.)
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
- `[packages] allow_remote = true` — without this, remote configs cannot install `[npm]/[pip]/[cargo]/[nuget]/[gem]/[go]` entries.
- Project-config telemetry overrides (endpoint) — refused with stderr warning. Endpoint override only via `JARVY_OTLP_ENDPOINT`.

Package-name validation (`validate_package_name` / `validate_package_version`) refuses leading-`-`, URL schemes, shell-meta, and control bytes (ESC/BEL/DEL/NUL — closes ANSI injection in dry-run preview). `jarvy validate` runs them on every `[npm]/[pip]/[cargo]/[nuget]/[gem]/[go]` entry.

### Config Files

- `jarvy.toml` (project) — tools, packages, roles, hooks, etc.
- `~/.jarvy/config.toml` (global) — telemetry, update, machine fingerprint
- `.jarvy/state.json` (project) — drift baseline

### Bootstrap script

`scripts/bootstrap.sh` is the canonical one-command onboarding entry point. Reusable: end-user repos can copy it into their own `scripts/` so contributors run `./scripts/bootstrap.sh` to install Jarvy (via `dist/scripts/install.sh`) and execute `jarvy setup` against the repo-root `jarvy.toml`. Idempotent. Flags: `--no-setup`, `--channel <stable|beta|nightly>`, passthrough args to `jarvy setup`. When recommending Jarvy integration to an end user, prefer pointing them at this script over hand-rolled curl-pipe + `cargo install` snippets.

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
| `packages.phase_started` / `packages.phase_completed` | run_packages_phase | Carries `dry_run`, `npm`, `pip`, `cargo`, `nuget`, `gem`, `go` booleans, `duration_ms` |
| `packages.phase_skipped` | early-return | `reason`, `dry_run` |
| `packages.phase_previewed` | dry-run preview | Per-ecosystem `*_count` (renamed from `packages.dry_run`) |
| `packages.remote_refused` | trust gate refusal | `reason` (e.g. `allow_remote_packages_not_set`) |
| `packages.install_failed` | install_packages outer error | One per ecosystem on failure |
| `package.requested` | per-package entry | `ecosystem`, `package`, `version`, `platform` |
| `package.installed` | per-package success | + `duration_ms` |
| `package.failed` | per-package failure | `error!` level; `error` redacted |
| `package_command.failed` | run_package_command non-zero | + `stderr_tail` (last 4KB) |
| `commands.extras_refused_keys` | `[commands]` key sanitizer (`config::sanitize_extras_keys` — shared by interactive menu AND `jarvy run` via `read_commands_config`) | `count` (control-byte / Trojan-Source keys dropped). Gated. |
| `run.command.start` / `run.command.complete` / `run.command.failed` | `jarvy run <name>` (npm-run-style `[commands]` runner) | `label` (display-sanitized, same field name as `interactive.command.*` so the two domains join), `cmd_hash` (shared `short_cmd_hash` — command text never emitted); start adds `well_known` (run/test/setup slot vs extra), `extra_args_count`; complete adds `exit_code` (`-1` sentinel = signal-killed, matches interactive; process exit stays 1), `duration_ms`; failed adds `error` |
| `run.command.not_found` | `jarvy run <name>` with no matching `[commands]` entry | `label`. Warn. |
| `run.command.refused` | `jarvy run` refused execution | `label`, `reason = "nul_byte" \| "percent_windows"` (cmd.exe expands `%VAR%` even inside quotes — no escape exists, so `%`-bearing `--` args are refused on Windows). Warn. |
| `run.command.config_error` | `jarvy run` couldn't load jarvy.toml | `error_kind = "missing" \| "unreadable" \| "parse"`. Warn. |
| `run.command.list` | bare `jarvy run` (listing mode) | `format`, `count` — execute-vs-discovery split. |
| `shell_init.generated` | `jarvy shell-init` emitted a snippet | `shell` (bash/zsh/fish/sh/powershell/nushell) — per-shell adoption. |
| `completions.generated` | `jarvy completions <shell>` generated a script | `shell` — per-shell adoption. |
| `ai_hook.phase_started` / `ai_hook.phase_completed` | apply phase | `agents`, `applied`, `refused_local`, `refused_remote`, `failures`, `duration_ms` |
| `ai_hook.agent_applied` / `ai_hook.agent_failed` | per agent | `agent_failed` carries `error_type` only — formatted message NOT emitted (no user-content leak) |
| `ai_hook.provisioned` | per hook | `agent`, `hook_name`, `library_source` |
| `ai_hook.custom_refused_summary` | trust gate | `local_count`, `remote_count` — counts only, no names/bodies |
| `ai_hook.check_completed` | drift check | `agents_checked`, `drifted_agents` |
| `ai_hook.windows_auto_translated` | Bash→PowerShell xlat | `agent`, `hook_name` |
| `mcp_register.phase_started` / `mcp_register.phase_completed` | apply phase | `agents`, `applied`, `refused_local`, `refused_remote`, `failures`, `duration_ms` |
| `mcp_register.agent_applied` / `mcp_register.agent_failed` | per agent | `agent_failed`: `error_type` only |
| `mcp_register.auto_detected` | `jarvy setup` synthesizes a default `[mcp_register]` because the project had no block and at least one AI agent was detected on disk | `count`, `agents` (comma-joined slugs), `platform`. Does NOT fire when an explicit `[mcp_register]` block is present, in dry-run, in test mode, in seamless / CI sandboxes, or when `JARVY_MCP_REGISTER=0`. |
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
| `git_hooks.phase_started` / `git_hooks.phase_completed` | setup `run_git_hooks_phase` (PRD-048) | `dry_run`, `installed`, `duration_ms` |
| `git_hooks.phase_skipped` | no framework detected | `reason` |
| `git_hooks.phase_previewed` | dry-run preview | `framework` |
| `git_hooks.remote_refused` | remote config tried auto_install | `reason = "allow_remote_not_set"` |
| `git_hooks.install_failed` | install_hooks returned error | `error_kind`, `error` |
| `git_hooks.installed` / `git_hooks.updated` | per-framework op | `framework = "pre-commit"`, `install_hooks` (for installed) |
| `git_hooks.install_started` / `git_hooks.install_completed` | `install_hooks()` entry/exit (CLI + setup callers; obs P1 review item 23/24) | started: `enabled`, `auto_update`, `run_after_install`. completed: `status = "applied" \| "skipped" \| "failed"`, `applied`, `framework`, `auto_update`, `run_after_install`, `duration_ms` |
| `git_hooks.update_started` / `git_hooks.update_completed` | `update_hooks()` entry/exit | started: `enabled`. completed: `status`, `applied`, `framework`, `duration_ms` |
| `git_hooks.pre_commit_version_mismatch` | version pin triggered upgrade | `installed`, `required` |
| `git_config.phase_started` / `git_config.phase_completed` | setup `run_git_phase` | started: `remote`, `scope`. completed: `ok`, `error_kind` (bounded: `git_not_installed`/`refused`/`write_failed`/`io`/`invalid_key`/…/`none`), `remote`, `scope`, `signing`, `aliases`, `extra_key_count`, `os_defaults_enabled`, `duration_ms`. |
| `git_config.phase_previewed` | dry-run git phase | `remote`, `scope`, `os_defaults_enabled`, `extra_key_count`. |
| `git_config.remote_refused` | remote-origin config applied `[git]` without `allow_remote` | `reason = "allow_remote_not_set"`. Warn. |
| `git_config.extra_applied` | `[git.extra]` keys written (summary) | `key_count`, `sections` (comma-joined DISTINCT section prefixes — never values/full keys). |
| `git_config.extra_key_applied` | per `[git.extra]` key, AFTER its write (partial-failure trail) | `section` (prefix only). Debug. |
| `git_config.os_defaults_applied` | `configure_os_defaults` (both branches — opt-out graphable) | `enabled`, `opted_out`, `keys_written`. |
| `git_config.os_default_key_applied` | per default key, AFTER its write succeeds | `key`, `value` (both jarvy constants). Debug. |
| `git_config.exec_key_refused` / `git_config.exec_key_override_applied` | `[git.extra]` key whose value git executes — refused, or applied under `JARVY_ALLOW_GIT_EXEC_KEYS` | `key` (never value). Warn. |
| `git_config.exec_value_refused` | typed `editor`/`credential_helper` (shell-interpreted key) value with a `!` / shell metacharacter / program path | `key`. Warn. |
| `git_config.protect_downgrade_refused` / `git_config.protect_downgrade_override_applied` | `[git.extra]` value weakens a guardrail — refused, or applied under `JARVY_ALLOW_GIT_PROTECT_DOWNGRADE` | `key`, `guardrail = "protect_ntfs_hfs" \| "safe_directory_wildcard" \| "safe_bare_repository" \| "fsck_objects_disabled" \| "fsck_ignore" \| "tls_verify_disabled"`. Warn. |
| `git_config.shell_escape_refused` / `git_config.shell_alias_refused` | `!`-shell value / alias refused | `key` / `alias`. Warn. |
| `git_config.refused_invalid_key` / `git_config.refused_option_value` | `[git.extra]` key grammar / leading-`-` value refused | `key`. Warn. |
| `git_config.set_failed` | `git config` write exited non-zero | `key`, `error_brief` (first line of git stderr, ≤160 chars). Warn. |
| `library.sync.started` | per-source fetch begins (PRD-054) | `url` (redacted), `require_signature` |
| `library.sync.completed` | fetch + parse OK | `url`, `items_synced`, `ai_hook_count`, `mcp_server_count`, `skill_count`, `from_cache`, `signature_verified` |
| `library.fetch.cached_hit` | served from disk cache after network failure | `url`, `reason = "fetch_failed"` |
| `library.cache.write_failed` | disk-write best-effort failure (non-fatal) | `url`, `error` |
| `library.signature_disabled` | `require_signature = false` warning | `url` |
| `library.remote_refused` | trust gate — remote config declared `library_sources` | `consumer = "ai_hooks" \| "mcp_register" \| "skills"`, `reason` |
| `skills.installed` | per-skill install success | `skill`, `version`, `agent_count`, `skipped_count` |
| `skills.updated` | per-skill `jarvy skills update` success (PRD-049 phase 2) | `skill`, `version`, `updated_count`, `unchanged_count`, `skipped_count`. Emitted even when everything was unchanged (`updated_count = 0`) so no-op runs are graphable. |
| `skills.removed` | per-skill `jarvy skills remove` success | `skill`, `removed_count`, `absent_count`. Idempotent re-removes emit with `removed_count = 0`. |
| `library.git.clone_started` | begin git clone (PRD-055) | `repo` (redacted), `git_ref` |
| `library.git.clone_completed` | clone + SKILL.md walk succeeded | `repo`, `git_ref`, `subpath`, `skills_discovered`, `duration_ms` |
| `library.git.clone_failed` | `git` subprocess exit nonzero | `args` (per-arg redacted — review item 6), `exit`, `error` (redacted) |
| `library.git.cache_hit` | served from clone cache after git failure | `url`, `reason = "git_failed"` |
| `library.git.mutable_ref` | branch ref pinned (publishers can rev silently) | `repo`, `git_ref`, `advice` |
| `library.git.missing_git` | `git` CLI not on PATH | `os` |
| `library.git.symlink_skipped` | symlink in cloned repo refused (review item 2) | `path` (relative to clone root) |
| `library.git.path_escape_refused` | SKILL.md canonicalizes outside clone root (review item 2 defense-in-depth) | `canon_path` |
| `library.git_skill.skipped` | SKILL.md missing required frontmatter | `path`, `reason` (missing name / version / parse fail) |
| `library.file_url_refused` | `file://` URL points outside cache root (review item 3) | `reason = "outside_cache_root"` |
| `library.sync.failed` | every sync error path (review item 8) | `url`, `scheme = "manifest" \| "git"`, `error_kind`, `error` |
| `library.companion.fetched` | companion artifact fetched + sha-verified (hook `bash_url` / `powershell_url` bodies, skill `companion_files`) | `url` (redacted), `bytes`, `from_cache`. Debug level on content-addressed cache hits, info on fresh fetches. |
| `library.companion.sha_mismatch` | fetched companion body doesn't match the manifest sha pin | `url`, `expected`, `actual`. Error level. |
| `library.companion.fetch_failed` | every other companion fetch error path (invalid pin, network, non-HTTPS, non-UTF-8 script body) | `url`, `error_kind`, `error`. Warn level. |
| `library.companion.refused_filename` | skill `companion_files` filename failed path-safety validation (traversal / absolute / control bytes / jarvy-owned name) | `skill`, `reason` (filename NOT logged — it's attacker-controllable, mirrors `workspace.member_invalid`). Warn level. |
| `library.signature.verified` | `require_signature = true` + cosign-verified successfully (PRD-054 phase 5) | `url`. Per-fetch — cached hits don't re-emit. |
| `discover.applied` | `jarvy discover --apply` succeeded (PRD-044) | `tools_added`, `recommended_added`, `already_configured`, `recommended_dropped_dup` (count of companions suppressed because also required), `detections_by_rule` (comma-joined rule names), `target = "merged" \| "noop" \| "bailed_to_fresh"`, `duration_ms` |
| `discover.setup_advisory` | `jarvy setup` ran the continuous-discovery scan and found new tools (PRD-044 phase 2) | `new_tools`, `uninstallable` |
| `discover.recommended_dropped` | per-companion suppression when the same tool is required via own-marker (dedup dashboard trail) | `name`, `promoted_to = "required"`. Debug level. |
| `discover.rules_loaded` | one-shot per process at first `discover` invocation — count of loaded detection rules | `default_rule_count`, `custom_rule_count`, `total_rule_count`. Info level. Guards against `#[cfg]` regressions silently dropping rules. |
| `discover.jarvy_toml_perms_unsafe` | chmod on discover-written `jarvy.toml` failed or was silently ignored (NFS/drvfs/exFAT) | `target`, `error` OR `mode`, `fs_hint = "chmod_failed" \| "chmod_ignored"`. Warn level. |
| `discover.sensitive_key_refused` | discover refused to write a config carrying a top-level `[secrets]` / `[credentials]` / `[tokens]` / `[api_keys]` / `[auth]` section — case-insensitive | `key` (original case), `key_lower`. Error level — invariant breach on the 0644 chmod policy. |
| `workspace.validate_completed` | `jarvy workspace validate` finished (PRD-047) | `status = "ok" \| "warnings" \| "invalid"`, `members`, `errors`, `warnings`, `duration_ms`. `warn!` level when `errors > 0`, else `info!` |
| `workspace.member_invalid` | per-member validation failure | `error_kind = "escapes_workspace_root" \| "dir_missing" \| "toml_parse_fail"` (member name NOT logged — it's attacker-controllable in a hostile root config) |
| `wizard.started` | `jarvy wizard` start (PRD-056) | `mode = "headless" \| "skill_drop" \| "quickstart_fallback"`, `agent`, `apply`, `skill_only` |
| `wizard.skill_dropped` | wizard wrote a SKILL.md to the agent's skills dir | `agent`, `skill_path` |
| `wizard.headless_spawned` | wizard spawned the agent's CLI in headless mode | `agent`, `cmd_argv0` (argv[0] only — args carry the prompt body), `mcp_preapproval` (allowlist scope passed via `--allowedTools`, or `""` when none), `wizard_session_env = true` (marker that `JARVY_WIZARD_SESSION=1` was set on the child), `wizard_session_id` (per-invocation UUID for correlation) |
| `wizard.headless_exit` | agent CLI returned | `agent`, `exit_code`, `wall_ms`, `jarvy_toml_before = "absent" \| "present"`, `jarvy_toml_after = "absent" \| "created" \| "unchanged" \| "modified"`, `terminal_state = "playbook_completed" \| "noop_already_configured" \| "early_exit" \| "unknown"`, `wizard_session_id` |
| `wizard.refused` | trust-boundary or runtime refusal | `reason = "sandbox" \| "ci" \| "non_tty" \| "remote_config" \| "no_agent_installed" \| "skill_drop_failed" \| "headless_spawn_failed"` |
| `wizard.session_token_activate_failed` | `WizardSessionGuard::activate` couldn't write the marker file (perms error, disk full, read-only FS) — wizard runs on with a no-op guard | `error`, `path`. Warn level. |
| `wizard.session_token_perms_unsafe` | marker file chmod to 0600 failed OR was silently ignored — mirrors the `discover.jarvy_toml_perms_unsafe` pattern | `path`, `error` OR `mode`, `fs_hint = "chmod_failed" \| "chmod_ignored"`. Warn level. Marker's 0600 is a capability boundary — silent chmod ignore lets other local users forge tokens. |
| `wizard.session.bypass_refused` | `session::is_active()` returned false while gating an MCP mutation — distinguishes "user typed no" from "orphaned descendant of a killed wizard tried to bypass" | `reason = "env_missing" \| "env_empty" \| "no_home" \| "marker_missing" \| "mtime_unavailable" \| "marker_stale" \| "marker_future_mtime"`, `session_id`. Debug level. |
| `mcp.mutation.wizard_bypass` | wizard-driven MCP mutation skipped the TTY confirmation (see `src/mcp/extended_tools.rs::gate_mutation`) | `tool`, `client` (`claude-code` \| `codex` only — unexpected clients fall through), `workspace`, `effect` (ANSI-sanitized), `pid`, `wizard_session_id`. Info level. |
| `mcp.mutation.wizard_bypass_unexpected_client` | env + marker present but MCP client is NOT `claude-code` / `codex` — bypass does NOT fire; forensic warning of a compromised same-user MCP client | `tool`, `client`, `workspace`, `wizard_session_id`. Warn level. |

**Telemetry gate.** Every `library.*`, `library.git.*`, `library.git_skill.*`, `skills.*`, `git_hooks.*`, `package.*`, `discover.*`, `workspace.*`, `wizard.*`, `run.command.*`, `commands.*`, `shell_init.*`, `completions.*`, and `mcp.mutation.*` / `mcp.auto_approve.*` event reads `observability::telemetry_gate::is_enabled()` before emitting. Users with `telemetry.enabled = false` don't ship event breadcrumbs even when the OTLP exporter is otherwise configured. Review item 7 (P0) — previously the new `library.git.*` / `skills.*` / `git_hooks.*` domains bypassed the gate; now consistent with `packages.*`. The `mcp.auto_approve.*` domain was added to this list in the follow-up review's Obs F11 fix. **ALL `git_config.*` events are gated** — lifecycle, adoption, AND the security refusals/overrides — via the module-local `gated_warn!`/`gated_info!`/`gated_debug!` macros in `src/git/setup.rs` (and the inline `if is_enabled()` in `run_git_phase`). Round-2 review corrected an earlier attempt to leave the security refusals ungated "for local audit": `tracing` has no sink split, so ungated = shipped to OTLP regardless — a `telemetry.enabled = false` consent breach. The user still sees every refusal via the returned `Err` → stderr, so gating costs no user-facing signal. Do NOT add a `git_config.*` `tracing::…!` that bypasses the macros.

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

## Structured output (`--format json`)

Every command that prints to stdout accepts `--format json` (PRD-051). The
JSON is pretty-printed by default and routed through either the canonical
`output::Outputable` trait (for handlers that ship a typed result struct)
or an inline `serde_json::json!()` envelope. CLI exit codes are identical
between human and JSON paths so `jq .` pipelines can rely on `$?` for
control flow. Commands with subcommand actions (`drift`, `logs`, `ticket`,
`services`, `workspace`) carry `--format` on each subcommand rather than
the parent; this matches the existing pattern set by `drift check` /
`logs view` and keeps clap parsing unsurprising.

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
