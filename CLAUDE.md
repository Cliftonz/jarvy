# CLAUDE.md

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

- **`src/main.rs`** - CLI entry point using clap with derive macros. Commands: `setup`, `bootstrap`, `configure`, `get`
- **`src/config.rs`** - Parses `jarvy.toml` using serde. Supports simple (`git = "2.40"`) and detailed (`git = { version = "2.40", version_manager = true }`) formats
- **`src/tools/registry.rs`** - Global `OnceLock<RwLock<HashMap>>` registry mapping tool names to handler functions
- **`src/tools/common.rs`** - Shared utilities: `Os` enum, `InstallError` type, `run()`, `has()`, `cmd_satisfies()`, package manager detection
- **`src/tools/spec.rs`** - ToolSpec pattern: `ToolSpec` struct and `define_tool!` macro for declarative tool definitions

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

### Config Files

- **`jarvy.toml`** (project) - Tools to provision with versions
- **`~/.jarvy/config.toml`** (global) - Telemetry settings, machine fingerprint

### Telemetry

Optional PostHog analytics + OTLP tracing. Configurable via `~/.jarvy/config.toml` or env vars (`JARVY_OTLP_ENDPOINT`).

## Testing

Integration tests are in `/tests/`. Key test env vars:
- `JARVY_TEST_MODE=1` - Disables interactive prompts
- `JARVY_FAST_TEST` - Skips external command execution

## Exit Codes

- `0` - Success
- `2` - CONFIG_ERROR (malformed jarvy.toml)
- `3` - PREREQ_MISSING (package manager not found)
- `5` - PERMISSION_REQUIRED (sudo needed)

## Conventions

- Rust 2024 edition idioms
- Conventional Commits: `feat:`, `fix:`, `docs:`, `chore:`, `refactor:`, `test:`
- Prefer stdlib and existing dependencies over new crates
- Run `cargo fmt` and `cargo clippy` before committing
