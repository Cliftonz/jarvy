# CLAUDE.md

@SKILL.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build Commands

```bash
cargo build                                    # Debug build
cargo build --release                          # Release build
cargo fmt --all                                # Format code
cargo clippy --all-features -- -D warnings     # Lint (must pass for CI)
cargo check --verbose                          # Type check
cargo test --verbose -- --show-output          # Run all tests
cargo test --test cli_dispatch -- --show-output  # Run single integration test
cargo run -p cargo-jarvy -- new-tool <name>    # Scaffold a new tool
```

## Architecture

Jarvy is a cross-platform CLI tool that provisions development environments from a `jarvy.toml` config file. It uses native package managers (Homebrew on macOS, apt/dnf/etc on Linux, winget/Chocolatey on Windows).

### Core Modules

- **`src/main.rs`** - Minimal entry point (~540 lines). Parses CLI args, initializes telemetry, dispatches to command handlers
- **`src/cli/`** - CLI argument parsing (PRD-037 refactor):
  - `args.rs` - `Cli` struct, `Commands` enum, `OutputFormat`, parse functions
  - `subcommands.rs` - Nested enums: `TemplatesSubcommand`, `TelemetryAction`, `ServicesAction`, `TeamAction`, `LockAction`, `ConfigAction`, `UpdateSubcommand`
- **`src/commands/`** - Command handlers extracted from main.rs:
  - `setup_cmd.rs` - Setup command (~500 lines, parallel install, hooks, services)
  - `get.rs`, `tools_cmd.rs`, `env_cmd.rs`, `ci_cmd.rs`, `services_cmd.rs`
  - `mcp_cmd.rs`, `telemetry_cmd.rs`, `team_cmd.rs`, `lock_cmd.rs`, `config_cmd.rs`
  - `roles_cmd.rs`, `bootstrap_cmd.rs`, `configure_cmd.rs`
  - `logs_cmd.rs`, `ticket_cmd.rs` - Logging and debug ticket commands (PRD-050)
- **`src/remote.rs`** - Remote config fetching with caching (`fetch_remote_config`, `transform_github_url`)
- **`src/interactive.rs`** - Interactive menu for users who run `jarvy` without subcommand
- **`src/config.rs`** - Parses `jarvy.toml` using serde. Supports simple (`git = "2.40"`) and detailed (`git = { version = "2.40", version_manager = true }`) formats
- **`src/roles/`** - Role-based configurations with inheritance (PRD-033). Key files: `definition.rs` (types), `resolver.rs` (inheritance), `commands.rs` (CLI)
- **`src/tools/registry.rs`** - Global `OnceLock<RwLock<HashMap>>` registry mapping tool names to handler functions
- **`src/tools/common.rs`** - Shared utilities: `Os` enum, `InstallError` type, `run()`, `has()`, `cmd_satisfies()`, package manager detection
- **`src/tools/spec.rs`** - ToolSpec pattern: `ToolSpec` struct and `define_tool!` macro for declarative tool definitions
- **`src/packages/`** - Language package dependencies (npm, pip, cargo) with virtual environment support (PRD-039)
- **`src/logging/`** - Thin re-export layer over `src/observability/` with helpers for log file management. Re-exports `LogConfig`, `LogFormat`, `LogLevel`, `Sanitizer`. Provides `default_log_directory()`, `read_recent_logs()`, `get_log_stats()`, `clean_logs()`. Actual writer/rotation/sanitizer implementations live in `src/observability/`.
- **`src/ticket/`** - Debug ticket generation (PRD-050). Key files: `collector.rs` (SystemInfo, ToolInfo), `bundler.rs` (ZIP archive creation)

### Tool Implementation Pattern

Tools use the declarative `define_tool!` macro for minimal boilerplate:

```rust
//! jq - JSON processor
use crate::define_tool;

define_tool!(JQ, {
    command: "jq",
    macos: { brew: "jq" },
    linux: { uniform: "jq" },
    windows: { winget: "jqlang.jq" },
});
```

Each tool lives in `src/tools/{name}/` with two files:
- `mod.rs` - Re-exports with `pub use {name}::*;`
- `{name}.rs` - Tool definition using `define_tool!` macro

**Macro variants:**
- `macos: { brew: "pkg" }` - Homebrew formula
- `macos: { cask: "pkg" }` - Homebrew cask (GUI apps)
- `linux: { uniform: "pkg" }` - Same package name across all distros
- `linux: { apt: "x", dnf: "y", pacman: "z", apk: "w" }` - Different names per package manager
- `windows: { winget: "Publisher.Package" }` - Winget package ID
- `custom_install: Some(fn_name)` - For tools needing shell scripts (nvm, rustup, brew)
- `default_hook: { description: "...", script: "..." }` - Post-install hook that runs automatically

Tools are registered in `src/tools/mod.rs` via `register_all()`.

### Default Hooks

Tools can define built-in post-install hooks that configure the tool after installation. Default hooks are:
- **Idempotent** - Safe to run multiple times (scripts check before modifying files)
- **Advisory** - Failures are warnings, not errors; setup continues
- **Overridable** - User-defined `[hooks.tool]` takes precedence

Example tool with default hook:
```rust
define_tool!(STARSHIP, {
    command: "starship",
    macos: { brew: "starship" },
    linux: { uniform: "starship" },
    windows: { winget: "Starship.Starship" },
    default_hook: {
        description: "Add starship shell initialization to .bashrc and .zshrc",
        script: r#"
# Add to .zshrc if not present
if [ -f "$HOME/.zshrc" ] && ! grep -q 'starship init zsh' "$HOME/.zshrc"; then
    echo 'eval "$(starship init zsh)"' >> "$HOME/.zshrc"
fi
"#
    },
});
```

List tools with default hooks: `jarvy tools --default-hooks`

### Tool Dependencies

Tools can declare dependencies on other tools to ensure correct installation order:

**Strict Dependencies** (`depends_on`): ALL listed tools MUST be available.
```rust
define_tool!(LAZYDOCKER, {
    command: "lazydocker",
    macos: { brew: "lazydocker" },
    // Docker TUI requires Docker daemon
    depends_on: &["docker"],
});
```

**Flexible Dependencies** (`depends_on_one_of`): AT LEAST ONE tool must be available.
```rust
define_tool!(KUBECTL, {
    command: "kubectl",
    macos: { brew: "kubectl" },
    // Needs any K8s cluster provider (minikube, kind, docker, etc.)
    depends_on_one_of: &["minikube", "kind", "k3d", "docker", "podman"],
});
```

**Dependency Behavior:**
- Strict: Missing deps cause warnings; tools are still installed but may not work
- Flexible: If one option is installed, satisfied. If one is in config, it's installed first. Otherwise, advisory warning
- Dependencies affect installation order via topological sort

**Dependency Functions** (`src/tools/spec.rs`):
- `get_tool_dependencies()` - Get strict dependencies
- `get_tool_flexible_dependencies()` - Get flexible dependencies
- `check_tool_dependencies()` - Returns `DependencyCheckResult` (Satisfied, MissingRequired, WillInstallFlexible, MissingFlexible)
- `order_tools_by_dependencies()` - Topological sort respecting both dependency types

### Role-Based Configurations

Jarvy supports role-based tool configurations for teams with diverse developer roles. Each role defines a tool set that gets merged with directly configured tools.

**Config Example:**
```toml
# Assign a role
role = "frontend"
# Or multiple roles (last wins for conflicts)
role = ["frontend", "devops"]

# Direct tools always override role tools
[provisioner]
vim = "latest"

# Role definitions
[roles.base]
description = "Base development tools"
tools = ["git", "docker"]

[roles.frontend]
description = "Frontend development"
extends = "base"                    # Inherit from parent
tools = ["node", "bun"]

[roles.frontend.tools]              # Version overrides
node = "20"
bun = "latest"

[roles.senior-frontend]
extends = "frontend"                # Deep inheritance (max 5 levels)
tools = ["kubectl"]
```

**CLI Commands:**
```bash
jarvy roles list                    # List available roles
jarvy roles list -v                 # Verbose with tool counts
jarvy roles show <name>             # Show role details
jarvy roles show <name> --resolved  # Show with inherited tools
jarvy roles show <name> --inheritance  # Show inheritance chain
jarvy roles diff <a> <b>            # Compare two roles
jarvy setup --role <name>           # Override role for single run
```

**Module**: `src/roles/` - Role definitions, resolution with inheritance, and CLI commands.

### Language Package Dependencies

Jarvy supports installing language-specific packages alongside CLI tools via `[npm]`, `[pip]`, `[cargo]`, and `[nuget]` sections.

**Module**: `src/packages/` - Language package dependency management.

**Key Files**:
- `mod.rs` - `install_packages()` orchestration function
- `config.rs` - PackagesConfig, NpmConfig, PipConfig, CargoConfig, NugetConfig, PackageSpec
- `common.rs` - PackageError, run_package_command(), command_exists(), validate_package_name(), validate_package_version()
- `npm.rs` - NpmHandler with package manager auto-detection
- `pip.rs` - PipHandler with virtual environment support
- `cargo_pkg.rs` - CargoHandler for cargo install
- `nuget.rs` - NugetHandler for `dotnet tool update -g` (idempotent global tool install)

**Configuration** (`jarvy.toml`):
```toml
[npm]
typescript = "^5.0"
eslint = "latest"
package_manager = "pnpm"    # Auto-detected from lock file if not set
from_lockfile = false       # Install from package-lock.json instead

[pip]
pytest = ">=7.0"
black = "latest"
venv = ".venv"              # Virtual environment path
create_venv = true          # Auto-create venv if missing
from_lockfile = false       # Install from requirements.txt instead

[cargo]
cargo-watch = "latest"
cargo-nextest = "0.9"
locked = true               # Use --locked flag

[nuget]
dotnet-ef = "latest"        # EF Core CLI
csharpier = "0.30.0"        # C# formatter
dotnet-format = "latest"
```

**Package Spec Variants**:
- Simple: `typescript = "^5.0"`
- Detailed: `some-crate = { version = "1.0", optional = true, features = ["feature1"] }`

**Package Manager Detection** (npm):
- Auto-detects from lock files: `pnpm-lock.yaml` → pnpm, `yarn.lock` → yarn
- Explicit override: `package_manager = "yarn"`

**Virtual Environment** (pip):
- Creates `.venv` if `venv` is set and `create_venv = true`
- Shows activation hint after setup
- Supports `--system-site-packages`

**.NET global tools** (`[nuget]`):
- Installs CLI binaries via `dotnet tool update -g <name>` (the `update` verb is idempotent — `install -g` errors when the tool is already present).
- Scope is .NET global tools only. Project-level NuGet PackageReferences (the deps in a `.csproj` / `Directory.Packages.props`) are NOT managed here — those are restored by `dotnet restore` during build.
- Same `validate_package_name` / `validate_package_version` guards as other ecosystems: refuses leading-`-` names (cargo / npm / dotnet would honor them as flags), URL schemes, and shell-meta chars.

**Integration**: Package installation runs after tool hooks and before environment setup in `jarvy setup`. Order: npm → pip → cargo → nuget.

### Git Configuration

Jarvy can configure Git settings including user identity, commit signing, default branch, and aliases.

**Module**: `src/git/` - Git configuration automation

**Configuration** (`jarvy.toml`):
```toml
[git]
# User identity (plain string or from environment)
user_name = "John Doe"
user_email = { env = "GIT_EMAIL", default = "john@example.com" }

# Commit signing (SSH or GPG)
signing = true
signing_key = "~/.ssh/id_ed25519.pub"
signing_format = "ssh"  # or "gpg", auto-detected if not set

# Default settings
default_branch = "main"
pull_rebase = true
auto_stash = true
push_autosetup = true
editor = "vim"

# Line endings
autocrlf = "input"  # true, false, or input
eol = "lf"

# Credential helper (auto-detected by OS if not set)
credential_helper = "osxkeychain"

# Configuration scope
scope = "global"  # or "local"

# Git aliases
[git.aliases]
co = "checkout"
br = "branch"
ci = "commit"
st = "status"
lg = "log --oneline --graph --decorate"
```

**Key Types**:
- `GitConfig` - Main configuration struct
- `ConfigValue` - Plain string or `{ env = "VAR", default = "value" }`
- `ConfigScope` - `global` (~/.gitconfig) or `local` (.git/config)
- `SigningFormat` - `ssh` or `gpg`
- `AutoCrlf` - `true`, `false`, or `input`

**ConfigValue Resolution**:
- Plain: `user_name = "John"` → Uses value directly
- From Env: `user_email = { env = "GIT_EMAIL" }` → Reads from environment
- With Default: `user_email = { env = "GIT_EMAIL", default = "fallback@example.com" }`

**Signing Auto-Detection**:
- Keys ending in `.pub` → SSH signing
- Other keys → GPG signing

**Credential Helper Defaults**:
- macOS: `osxkeychain`
- Linux: `cache`
- Windows: `manager-core`

**Integration**: Git configuration runs after package installation and before environment setup in `jarvy setup`.

### AI Agent Hooks

Distributes guardrail hooks (e.g. `block-rm-rf`, `block-secrets-commit`) to every developer's AI coding agent — Claude Code, Cursor, Codex CLI, Windsurf, Cline, and Continue — from a single `[ai_hooks]` block.

**Module**: `src/ai_hooks/` — AI agent hook provisioning.

**Key Files**:
- `config.rs` — `AiHooksConfig`, `HookEntry`, `AgentTarget`, `HookScope`, `ConfigOrigin` (Local|Remote — drives the trust boundary)
- `event.rs` — Canonical `HookEvent` taxonomy (`pre_tool_use`, `post_tool_use`, ...) with per-agent mapping
- `library.rs` — 16 curated `LibraryHook` entries with both Bash and PowerShell script bodies
- `platform.rs` — Bash → PowerShell auto-translator for custom commands without a `command_windows` field
- `runner.rs` — Top-level `apply` / `check` / `remove` orchestration plus library lookup + custom-command audit
- `error.rs` — `AiHookError` (`UnknownLibraryHook`, `UnsupportedEvent`, `UnsupportedPlatform`, ...)
- `agents/mod.rs` — `AgentProvisioner` trait + `ResolvedEntry<'a>` (Cow-borrowed library bodies) + static dispatch table
- `agents/markers.rs` — `_jarvy_managed`, `_jarvy_sha256`, YAML fences, filename infix — consolidated
- `agents/json_merge.rs` — Shared `retain_non_jarvy_named` / `collect_marker_names` / `entry_hash` for the 4 JSON provisioners
- `agents/io.rs` — Atomic JSON / text / executable write (PID + nanos tempfile, refuses symlinks, mode 0o644/0o755)
- `agents/claude_code.rs` — Writes `~/.claude/settings.json` (user) or `.claude/settings.json` (project)
- `agents/cursor.rs` — Writes `~/.cursor/hooks.json` (user) or `.cursor/hooks.json` (project); shims Bash via `bash -c '...'`
- `agents/codex.rs` — Writes `~/.codex/hooks.json` with `commandWindows` field for cross-platform
- `agents/windsurf.rs` — Writes `~/.codeium/windsurf/hooks.json` with `command` + `powershell` fields
- `agents/cline.rs` — Writes executable fragments + dispatcher script under `~/Documents/Cline/Rules/Hooks/`; macOS/Linux only
- `agents/continue_dev.rs` — Writes declarative `~/.continue/permissions.yaml` glob deny list
- `commands/ai_hooks_cmd.rs` — `jarvy ai-hooks {list|apply|check|remove|test}` handlers

**Configuration** (`jarvy.toml`):
```toml
[ai_hooks]
agents = ["claude-code", "cursor", "codex", "windsurf", "cline", "continue"]
scope = "user"                          # user | project
allow_custom_commands = false           # gate raw `command = "..."` entries

[[ai_hooks.hook]]
use = "block-rm-rf"

[[ai_hooks.hook]]
use = "block-force-push"
agents = ["claude-code", "cursor"]      # optional: narrow to subset
```

**Curated library** (16 hooks):
- **Safety**: `block-rm-rf`, `block-git-reset-hard`, `block-force-push`, `block-protected-branch-commit`, `block-kubectl-delete`, `block-docker-prune`, `block-drop-table`, `block-prod-db-write`
- **Security**: `block-secrets-commit`, `block-edit-env-files`, `block-read-secret-files`, `block-cat-env-files`, `block-curl-bash-pipe`, `block-malware-install`
- **Compliance**: `audit-log` (writes `~/.jarvy/logs/ai-hooks-audit.jsonl`)
- **Policy**: `commit-message-format-guard`

Each ships a Bash and PowerShell variant; Jarvy emits the right one per agent + OS.

**CLI**:
```bash
jarvy ai-hooks list                  # show what's configured
jarvy ai-hooks list --library        # show all 16 library hooks
jarvy ai-hooks apply [--scope user|project]
jarvy ai-hooks check                 # detect drift (exit 1 if drift)
jarvy ai-hooks remove                # strip _jarvy_managed entries
jarvy ai-hooks test <name>           # inspect a library hook's scripts
```

**Trust model**:
- Library hooks (`use = "..."`) always allowed — vetted Jarvy source.
- Raw `command = "..."` entries refused unless `allow_custom_commands = true` AND `ConfigOrigin::Local`.
- Refusals emit `ai_hook.custom_refused_summary` telemetry (counts only — no names, no command bodies).
- Remote configs (loaded via `jarvy setup --from <url>`) are tagged `ConfigOrigin::Remote` and the runner refuses every raw `command` entry from them, regardless of the `allow_custom_commands` flag. The CLI flag is the only override.

**Idempotency**: Every Jarvy-managed entry carries a `_jarvy_managed` JSON marker. `apply` removes prior entries with the same name; `remove` strips every marker but preserves user-authored hooks.

**Cross-platform**:
- Claude Code / Cursor on Windows: bash hook wrapped in `powershell -NoProfile -Command "..."` shim.
- Codex / Windsurf: ship both variants in the same JSON entry; agent picks at runtime.
- Cline: Unix only — Windows targets skipped with warning.
- Continue: declarative globs, platform-independent.

**Telemetry events** (domain `ai_hook`, snake_case actions):
- `ai_hook.phase_started` — `agents`, `hooks_count`, `scope`, `dry_run`
- `ai_hook.phase_completed` — `applied`, `agents_touched`, `refused_local`, `refused_remote`, `failures`, `duration_ms`
- `ai_hook.agent_applied` — `agent`, `applied`, `warnings`, `settings_path` (redacted)
- `ai_hook.agent_failed` — `agent`, `error_type` (stable `AiHookError::kind()`). The formatted error message is NOT emitted — user-controlled hook names/reasons never leak to OTLP
- `ai_hook.provisioned` — `agent`, `hook_name`, `library_source`
- `ai_hook.custom_refused_summary` — `local_count`, `remote_count` (single INFO line per phase, not WARN per entry)
- `ai_hook.check_completed` — `agents_checked`, `drifted_agents`
- `ai_hook.windows_auto_translated` — `agent`, `hook_name`

**Integration**: AI hook provisioning runs after Git config and before environment setup in `jarvy setup`. Failures surface as warnings — setup continues.

**Docs**: `docs/ai-hooks.md`. **Example**: `examples/ai-hooks/jarvy.toml`.

### MCP Server Registration

Auto-registers the built-in Jarvy MCP server (and optional custom servers) with each developer's AI coding agents so they can discover Jarvy's tools without manual setup. Mirrors the AI hooks architecture and trust model.

**Module**: `src/mcp_register/` — registration provisioning.

**Key Files**:
- `config.rs` — `McpRegisterConfig`, `JarvyServerOverride`, `McpServerSpec`, `McpAgentTarget`, `McpRegistrationScope`, `McpServerTransport`. Origin-tagged via the shared `ConfigOrigin` from ai_hooks.
- `error.rs` — `McpRegisterError`; bridges from `AiHookError` so the shared io helpers in `ai_hooks::agents::io` can be reused with `?`.
- `runner.rs` — `apply` / `check` / `remove` orchestration plus the trust gate (`ConfigOrigin::Remote` cannot ship custom servers, period).
- `registrars/mod.rs` — `AgentRegistrar` trait, `ResolvedServer`, `ApplyOutcome` / `CheckOutcome` / `RemoveOutcome`, static dispatch table.
- `registrars/claude_code.rs` — `~/.claude.json` (user) / `.mcp.json` (project); JSON-merge with `_jarvy_managed_servers` marker array.
- `registrars/cursor.rs` — `~/.cursor/mcp.json` (user) / `.cursor/mcp.json` (project).
- `registrars/codex.rs` — `~/.codex/config.toml` (user) / `.codex/config.toml` (project) — TOML, not JSON. Uses `toml::Value` round-trip.
- `registrars/windsurf.rs` — `~/.codeium/windsurf/mcp_config.json` (user only — project requests fall back with warning).
- `registrars/cline.rs` — VS Code globalStorage path (`~/Library/Application Support/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json` on macOS; equivalent on Linux/Windows).
- `registrars/continue_dev.rs` — Per-server YAML files: `.continue/mcpServers/<name>.jarvy.yaml`. Removal = file deletion.
- `commands/mcp_register_cmd.rs` — `jarvy mcp-register {list|apply|check|remove}` CLI handler.

**Configuration** (`jarvy.toml`):
```toml
[mcp_register]
agents = ["claude-code", "cursor", "codex", "windsurf", "cline", "continue"]
scope = "user"                        # user | project
allow_custom_servers = false          # gate raw [[mcp_register.server]] entries

[mcp_register.jarvy]                  # optional override
command = "/opt/jarvy/bin/jarvy"
args = ["mcp"]

[[mcp_register.server]]               # optional; refused unless allow_custom_servers
name = "github"
transport = "stdio"
command = "gh-mcp-server"
agents = ["claude-code", "cursor"]
```

**CLI**:
```bash
jarvy mcp-register list                  # show config + on-disk state
jarvy mcp-register apply [--scope user|project]
jarvy mcp-register check                 # detect drift (exit 1 if drift)
jarvy mcp-register remove                # strip _jarvy_managed_servers entries
```

**Trust model**: Identical to ai_hooks. Built-in `jarvy` server always allowed. Custom servers refused unless `allow_custom_servers = true` AND `ConfigOrigin::Local`. Remote configs (fetched via `jarvy setup --from <url>`) cannot enable the flag; the runner refuses every custom entry from them at resolve time.

**Marker scheme**: For JSON-based agents, a `_jarvy_managed_servers: ["jarvy", ...]` array at the root tracks owned server names. Entries themselves stay schema-clean. For Continue.dev, the `.jarvy.yaml` filename suffix is the marker.

**Telemetry events**:
- `mcp_register.phase_started` — `agents`, `servers_count`, `scope`
- `mcp_register.phase_completed` — `applied`, `agents_touched`, `refused_local`, `refused_remote`, `failures`, `duration_ms`
- `mcp_register.agent_applied` — `agent`, `applied`, `settings_path` (redacted)
- `mcp_register.agent_failed` — `agent`, `error_type` (stable `McpRegisterError::kind()`). Formatted message NOT emitted.

**Integration**: Runs after the AI hooks phase, before the drift snapshot in `jarvy setup`.

**Tests**: 11 unit + 21 integration (`tests/mcp_register_integration.rs`) + 7 CLI e2e (`tests/mcp_register_cli.rs`) + 3 setup-phase. Covers all 6 agents, project + user scope, concurrent stress, corrupt prior settings, symlink refusal, per-agent failure isolation, Codex TOML round-trip, Windsurf project-scope fallback, drift detection round-trip.

**Docs**: `docs/mcp-registration.md`. **Example**: `examples/mcp-register/jarvy.toml`.

### MCP Extended Tools

Phase 2 of the MCP integration: beyond the tool-installer family (`jarvy_list_tools`, `jarvy_install_tool`, ...), the server exposes the broader Jarvy surface so AI agents can introspect AI hooks, MCP registration, drift, roles, services, templates, and config validation directly.

**Module**: `src/mcp/extended_tools.rs` (single file with all definitions + handlers).

**Tools added** (all `jarvy_` prefix):
- Read-only: `ai_hooks_list`, `ai_hooks_check`, `mcp_register_list`, `mcp_register_check`, `drift_check`, `drift_status`, `roles_list`, `roles_show`, `services_status`, `templates_list`, `templates_show`, `validate_config`
- Mutating (dry_run = true default): `ai_hooks_apply`, `mcp_register_apply`, `services_start`, `templates_use`

**Pattern**: each handler returns an MCP `content` envelope wrapping a JSON object. Read-only tools fail closed with `configured: false` / `baseline_exists: false` envelopes rather than JSON-RPC errors so agents can call them speculatively. Mutating tools default to `dry_run: true` and only require confirmation when set to false.

**Mutation guard** (`extended_tools::gate_mutation` + `MutationCtx`): every mutating handler runs rate-limit check → fail-closed stderr confirmation prompt → audit-log entry (`AuditAction::McpMutation`) before executing. The prompt returns `Err(user_cancelled)` when stderr is not a TTY so a headless agent cannot drive a mutation without a human in the loop.

**Workspace containment** (`safety::resolve_within_workspace`): caller-supplied paths (`services_start.project_dir`, `templates_use.output_path`) are resolved against the MCP workspace root and refused if they escape it. Defenses: canonicalize workspace, refuse absolute paths outside, refuse `..` traversal, refuse symlink at the endpoint. Workspace root comes from `JARVY_MCP_WORKSPACE` (absolute path env var) or falls back to the server's cwd at startup. `templates_use` additionally backs up any existing file to `<path>.bak` and writes atomically (tempfile w/ O_CREAT|O_EXCL → fsync → rename).

**Wiring**:
- `src/mcp/extended_tools.rs::extended_definitions()` appended to `src/mcp/tools.rs::list_tools()` so `tools/list` advertises them.
- `src/mcp/server.rs::McpServer` carries `workspace_root: PathBuf` (read from `JARVY_MCP_WORKSPACE` env or cwd) and exposes `mutation_ctx(client_name)`.
- `server.rs::handle_tools_call` dispatches `jarvy_*` names to the handlers; mutating arms build a `MutationCtx` and pass it in.

**Tests**:
- Unit tests in `extended_tools::tests` (library lookup, missing file, parse, templates list, drift baseline absence, services backend absence, audit-emitted-on-dry-run, workspace-containment for absolute / parent-dir / symlink / dotfile).
- E2E tests in `tests/mcp_extended_tools_integration.rs` — spawn the real `jarvy mcp` subprocess via `JARVY_MCP_WORKSPACE`-pinned harness, perform the MCP handshake, send `tools/list` and `tools/call` over JSON-RPC. Covers every dispatched tool name plus fail-closed-in-non-TTY-mode and over-the-wire workspace-escape refusal for `templates_use` and `services_start`.

**Docs**: section in `docs/mcp-server.md` (workspace + mutation guard documented).

### Configuration Drift Detection

Jarvy can detect when a developer's environment has drifted from the expected configuration after setup.

**Module**: `src/drift/` - Configuration drift detection and remediation.

**Key Files**:
- `config.rs` - DriftConfig, VersionPolicy (Major/Minor/Patch/Exact)
- `state.rs` - EnvironmentState, ToolState, SHA-256 file hashing
- `detector.rs` - DriftDetector, DriftReport, version comparison
- `reporter.rs` - Human-readable and JSON report output
- `fixer.rs` - DriftFixer for remediation

**Configuration** (`jarvy.toml`):
```toml
[drift]
enabled = true                 # Enable drift detection
check_on_run = false           # Check on every jarvy command
track_files = [".vscode/settings.json", "package.json"]
version_policy = "minor"       # major, minor, patch, exact
ignore_tools = ["vim", "neovim"]
allow_upgrades = true          # Only flag downgrades as drift
```

**Version Policy**:
- `major` - Only major version must match (1.x.x)
- `minor` - Major and minor must match (1.2.x) [default]
- `patch` - Exact patch version must match (1.2.3)
- `exact` - Exact version including pre-release/build metadata

**State File** (`.jarvy/state.json`):
```json
{
  "version": "1",
  "created_at": "1706086800Z",
  "updated_at": "1706086800Z",
  "config_hash": "sha256:abc123...",
  "tools": {
    "node": {
      "version": "20.10.0",
      "path": "/opt/homebrew/bin/node",
      "install_method": "brew"
    }
  },
  "files": {
    ".vscode/settings.json": "sha256:def456..."
  }
}
```

**CLI Commands**:
```bash
jarvy drift check              # Detect drift, exit 1 if found
jarvy drift check --format json  # JSON output for CI
jarvy drift status             # Show baseline state
jarvy drift status -v          # Verbose with paths/methods
jarvy drift accept             # Accept current state as baseline
jarvy drift accept --tools node,docker  # Accept specific tools
jarvy drift fix                # Remediate auto-fixable issues
jarvy drift fix --dry-run      # Preview what would be fixed
```

**Exit Codes**:
- `0` - No drift detected
- `1` - Drift detected
- `2` - No baseline state found

**Integration**: State is captured automatically after successful `jarvy setup`.

### Config Files

- **`jarvy.toml`** (project) - Tools to provision with versions
- **`~/.jarvy/config.toml`** (global) - Telemetry settings, machine fingerprint
- **`.jarvy/state.json`** (project) - Drift detection baseline state

### Network/Proxy Configuration

Jarvy supports corporate network environments with HTTP/HTTPS/SOCKS proxies and custom CA certificates.

**Module**: `src/network/` - Proxy resolution, credential handling, environment propagation.

**Configuration** (`jarvy.toml`):
```toml
[network]
https_proxy = "http://proxy.corp.com:8080"
no_proxy = ["localhost", "127.0.0.1", ".corp.com"]

[network.auth]
username = "jdoe"
password = { env = "PROXY_PASSWORD" }  # Or plain string (not recommended)

[network.tls]
ca_bundle = "/etc/ssl/certs/corporate-ca.crt"

[network.overrides.git]
https_proxy = "http://git-proxy.corp.com:8888"
```

**Priority Order**: Environment variables > Tool overrides > Global config

**Environment Variables Propagated**:
- `HTTP_PROXY`, `HTTPS_PROXY`, `NO_PROXY` (both cases)
- `CURL_CA_BUNDLE`, `SSL_CERT_FILE`, `NODE_EXTRA_CA_CERTS`, `GIT_SSL_CAINFO`

**Key Types**:
- `NetworkConfig` - Main proxy configuration
- `ProxyAuth` - Username/password with secure password sources
- `TlsConfig` - CA bundle and TLS settings
- `NetworkOverride` - Per-tool proxy settings

### Telemetry

Jarvy uses OpenTelemetry (OTEL) for unified telemetry: logs, metrics, and optional traces. Telemetry is **opt-in by default** (disabled until configured).

**Configuration** (`~/.jarvy/config.toml`):
```toml
[telemetry]
enabled = true                          # Master switch
endpoint = "http://localhost:4318"      # OTLP endpoint
protocol = "http"                       # "http" or "grpc"
logs = true                             # Log export
metrics = true                          # Metrics export
traces = false                          # Optional traces
sample_rate = 1.0                       # Trace sampling (0.0-1.0)
```

**Environment Variables** (override config file):
- `JARVY_TELEMETRY=1` - Enable/disable telemetry
- `JARVY_OTLP_ENDPOINT=http://...` - OTLP endpoint URL
- `JARVY_OTLP_PROTOCOL=grpc` - Protocol selection
- `JARVY_OTLP_LOGS=1`, `JARVY_OTLP_METRICS=1`, `JARVY_OTLP_TRACES=1`
- `JARVY_OTLP_SAMPLE_RATE=0.1` - Trace sampling rate

**CI Behavior**: Auto-disabled when `CI=true` unless `JARVY_TELEMETRY=1` is set.

**CLI Commands**:
```bash
jarvy telemetry status        # Show current config
jarvy telemetry enable        # Enable telemetry
jarvy telemetry disable       # Disable telemetry
jarvy telemetry set-endpoint <url>  # Set OTLP endpoint
jarvy telemetry test          # Test connectivity
jarvy telemetry preview       # Show what events would be sent
```

**Module**: `src/telemetry.rs` - Unified telemetry API with event functions and metrics.

**Event Taxonomy** — stable contract for AI agents and log queries:

| Event              | Source paths                          | Notes                                        |
|--------------------|---------------------------------------|----------------------------------------------|
| `tool.requested`   | per tool in config                    | Setup phase                                  |
| `tool.installed`   | per successful install                | Setup phase                                  |
| `tool.failed`      | per failed install                    | Setup phase                                  |
| `tool.unsupported` | setup unknown-tool loop + `--request` | Uniform field shape across both call sites   |
| `setup.started` / `setup.completed` | run lifecycle        | Carries duration, counts                     |
| `hook.started` / `hook.completed` / `hook.failed` / `hook.timeout` | per hook | |

`tool.unsupported` fields (uniform across setup and `--request`):
```
tool, version?, source, platform, suggestions, channel,
fallback_issue_url, scaffold_cmd, exit_code, opt_in_bypassed
```
- `source`: `config` | `mcp` | `cli` | `request`
- `channel`: `telemetry` | `manual` — telemetry is the canonical
  delivery channel; `manual` means the user must use the
  `fallback_issue_url` because telemetry is disabled
- `opt_in_bypassed`: `true` only on the `--request` path (the user
  typed the command, so consent is implicit and the OTEL counter
  fires regardless of the global opt-in). The `--request` path also
  emits `counter_fired` indicating whether the metric provider was
  initialized — when `false`, the channel falls back to `manual`
  and the GitHub URL is surfaced as the only remaining signal.
- `fallback_issue_url`: present only when `channel = "manual"`. The
  telemetry-on path omits it to keep log lines short.
- `exit_code`: always `8` (TOOL_UNSUPPORTED — see Exit Codes
  below). The setup-path emits one event per unknown tool, but the
  process only exits `8` when ALL configured tools are unknown;
  mixed runs return `0`.
- **Metric counter**: `jarvy.tool.unsupported` (one increment per
  event). Renamed from `jarvy.tool.not_supported` to match the event
  name — operators querying by event can find the counter without
  knowing the old name.

**Project-config trust boundary**: a `jarvy.toml` shipped with a cloned
repo can NARROW telemetry (disable, lower sample rate, drop signals)
but cannot BROADEN it (enable opt-in, change endpoint). Endpoint
overrides from project config are refused with a stderr warning;
override via `JARVY_OTLP_ENDPOINT` only.

### Self-Updating

Jarvy includes built-in self-updating functionality that can check for and install updates via multiple methods.

**Module**: `src/update/` - Self-updating with multiple installation methods and rollback support.

**Key Files**:
- `config.rs` - UpdateConfig, Channel (stable/beta/nightly), auto-install policies
- `method.rs` - InstallMethod detection (Homebrew, Cargo, apt, dnf, winget, etc.)
- `release.rs` - GitHub Releases API client
- `checker.rs` - Version checking with throttling
- `installer.rs` - Binary download and installation
- `rollback.rs` - Backup and rollback management
- `commands.rs` - CLI command handlers

**Configuration** (`~/.jarvy/config.toml`):
```toml
[update]
enabled = true                    # Enable auto-update checks
channel = "stable"                # stable, beta, nightly
check_interval = "24h"            # How often to check
auto_install = "never"            # never, patch-only, patch-minor, all
show_notifications = true         # Show update notifications
```

**Environment Variables**:
- `JARVY_UPDATE=0` - Disable update checks
- `JARVY_UPDATE_CHANNEL=beta` - Override release channel
- `JARVY_PINNED_VERSION=1.2.3` - Pin to specific version

**CI Behavior**: Auto-update checks are disabled in CI environments (`CI=true`).

**CLI Commands**:
```bash
jarvy update                      # Check and install latest update
jarvy update check                # Check for updates without installing
jarvy update --version 1.2.3      # Install specific version
jarvy update --channel beta       # Use beta channel
jarvy update --rollback           # Rollback to previous version
jarvy update history              # Show update history
jarvy update config               # Show update configuration
jarvy update enable               # Enable auto-updates
jarvy update disable              # Disable auto-updates
```

**Installation Methods Detected**:
- Homebrew (macOS)
- Cargo (Rust)
- apt (Debian/Ubuntu)
- dnf (Fedora/RHEL)
- pacman (Arch)
- winget (Windows)
- Chocolatey (Windows)
- Scoop (Windows)
- Binary (direct download fallback)

**Key Types**:
- `UpdateConfig` - Update configuration
- `Channel` - Release channel (Stable, Beta, Nightly)
- `InstallMethod` - Detected installation method
- `UpdateChecker` - Version checking with throttling
- `BinaryInstaller` - Direct binary download/install
- `RollbackManager` - Backup and restore previous versions

### Logging & Debug Tickets

Jarvy provides persistent file-based logging via the `tracing` ecosystem (with `tracing_appender::rolling` rotation and `tracing_appender::non_blocking` async file writes), plus debug ticket generation for support and diagnostics.

**Module**: `src/observability/` - Tracing subscriber setup, sanitizer, file/OTLP writers.
**Module**: `src/logging/` - Re-exports from observability + log file management helpers.
**Module**: `src/ticket/` - Debug ticket generation for support bundles.

**Key Files**:
- `observability/logging.rs` - `LogConfig`, `LogLevel`, `LogFormat` types and `default_log_directory()`
- `observability/sanitizer.rs` - Sensitive data redaction (API keys, tokens, passwords) using `Cow<'_, str>` to skip allocation when no match
- `analytics.rs` - tracing-subscriber wiring: console layer, `non_blocking` rolling file appender, optional OTLP layer; `shutdown_logging()` flushes the `SdkLoggerProvider` and the file `WorkerGuard` before `process::exit`
- `logging/mod.rs` - `read_recent_logs()`, `get_log_stats()`, `clean_logs()`, `format_size()`
- `ticket/collector.rs` - SystemInfo, ToolInfo collection
- `ticket/bundler.rs` - ZIP archive creation

**Configuration** (`jarvy.toml`):
```toml
[logging]
enabled = true
level = "info"           # error, warn, info, debug, trace
format = "json"          # text, json
max_file_size = "10MB"   # Rotate when exceeded
max_files = 5            # Keep N rotated files
max_age_days = 30        # Delete logs older than this
```

**CLI Commands**:
```bash
jarvy logs view [--lines N] [--level LEVEL]  # View recent logs
jarvy logs stats                              # Show log statistics
jarvy logs clean [--all]                      # Remove old/all logs
jarvy logs config                             # Show logging configuration

jarvy ticket create [--tool NAME]             # Generate diagnostic bundle
jarvy ticket show <id>                        # View ticket contents
jarvy ticket list                             # List existing tickets
jarvy ticket clean [--older-than DAYS]        # Remove expired tickets
```

**Log Directory**: `~/.jarvy/logs/` (jarvy.log, jarvy.log.1.gz, etc.)
**Tickets Directory**: `~/.jarvy/tickets/` (JARVY-YYYYMMDD-xxxxxxxx.zip)

**Key Types**:
- `LogConfig` - Logging configuration (level, format, directory)
- `Sanitizer` - Sensitive data redaction with regex patterns; exposes `sanitize_borrowed` returning `Cow<'_, str>` for zero-alloc on the no-match path
- `TicketData` - Complete ticket data structure
- `TicketCollector` - System and tool info collection
- `TicketBundler` - ZIP archive creation

**Rotation & flush**: rotation is handled by `tracing_appender::rolling` (daily rolls; old files deleted by `jarvy logs clean`). Writes are buffered through `tracing_appender::non_blocking`; the worker guard is held in `analytics::FILE_LOGGER_GUARD` and dropped during `shutdown_logging()` so buffered records flush before exit.

## Testing

Integration tests are in `/tests/`. Key test env vars:
- `JARVY_TEST_MODE=1` - Disables interactive prompts
- `JARVY_FAST_TEST` - Skips external command execution

## Exit Codes

- `0` - Success
- `2` - CONFIG_ERROR (malformed jarvy.toml)
- `3` - PREREQ_MISSING (package manager not found)
- `4` - NETWORK_TIMEOUT (network / proxy failure)
- `5` - PERMISSION_REQUIRED (sudo needed)
- `6` - INCOMPATIBLE_OS_ARCH (OS/arch unsupported for the action)
- `7` - HOOK_FAILED (pre_setup / post_install / post_setup hook errored)
- `8` - TOOL_UNSUPPORTED (every configured tool is unknown to the
  registry — mixed runs with at least one known tool still return 0)

## Conventions

- Rust 2024 edition idioms
- Conventional Commits: `feat:`, `fix:`, `docs:`, `chore:`, `refactor:`, `test:`
- Prefer stdlib and existing dependencies over new crates
- Run `cargo fmt` and `cargo clippy` before committing
