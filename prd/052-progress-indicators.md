# PRD-052: Progress Indicators for Long-Running Operations

## Overview

Add spinners and progress bars to Jarvy commands that perform long-running operations, giving users real-time feedback during tool installation, package installs, and drift checks.

## Problem Statement

Jarvy currently produces no visual feedback during potentially long operations:

- `jarvy setup` with 10+ tools can take minutes with no output until completion
- Package installs (`npm`, `pip`, `cargo`) run silently
- `jarvy audit` runs multiple scanners sequentially with no indication of progress
- `jarvy drift fix` can reinstall tools with no status updates

Users cannot distinguish between "working" and "hung" — especially when package managers are downloading large binaries.

## Evidence

- Almost every popular Rust CLI uses `indicatif` for progress: ripgrep, cargo, rustup, starship
- User confusion during multi-tool installs (no output = "is it broken?")
- CI environments need progress to avoid timeout kills (no output for 10min = job killed)

## Requirements

### Functional Requirements

1. **Spinners**: Show a spinner with current operation name during each tool install
2. **Multi-progress**: When installing tools in parallel (`--jobs N`), show N concurrent spinners
3. **Completion indicators**: Mark each tool as done/failed as it completes
4. **Summary line**: After setup, show "X installed, Y already satisfied, Z failed"
5. **CI mode**: Replace spinners with simple line-by-line output in CI (`--ci` or `CI=true`)

### Non-Functional Requirements

1. Spinners must not interfere with `--format json` output (disabled when JSON)
2. Spinners must not interfere with `--quiet` mode (disabled)
3. No additional runtime cost when output is redirected (detect `!isatty`)
4. Minimal dependency footprint (single crate)

## Non-Goals

- Full TUI dashboard (ncurses/ratatui)
- Download progress bars for individual files (package managers handle their own)
- Animated banners or ASCII art

## Implementation

### Crate Selection

**`indicatif` 0.17+** — the de facto standard for Rust CLI progress indication.

- `ProgressBar` for single-operation spinners
- `MultiProgress` for parallel tool installs
- `ProgressStyle` for customizable formats
- Built-in `isatty` detection

### Integration Points

| Location | Type | Description |
|----------|------|-------------|
| `src/commands/setup_cmd.rs` | Multi-spinner | One spinner per tool being installed |
| `src/provisioner.rs` | Spinner | Per-tool install progress |
| `src/packages/npm.rs` | Spinner | npm/yarn/pnpm install |
| `src/packages/pip.rs` | Spinner | pip install |
| `src/packages/cargo_pkg.rs` | Spinner | cargo install |
| `src/commands/audit.rs` | Spinner | Per-scanner progress |
| `src/commands/drift_cmd.rs` | Spinner | Drift check/fix progress |
| `src/hooks.rs` | Spinner | Hook execution |

### Spinner Style

```
[1/12] Installing node... ⠋
[2/12] Installing docker... ✓ (already installed)
[3/12] Installing python... ✓ (3.12.1)
```

### Cargo.toml Addition

```toml
indicatif = "0.17"
```

### Testing

- Unit tests verify spinner creation doesn't panic
- Integration tests use `--quiet` to suppress spinners
- CI tests verify output is clean (no ANSI escape sequences) when `CI=true`

## Effort Estimate

3-4 days

- Day 1: Add `indicatif`, create progress helper module
- Day 2: Integrate with setup_cmd and provisioner (parallel installs)
- Day 3: Integrate with packages, audit, drift, hooks
- Day 4: CI mode, `--quiet` compat, testing

## Dependencies

- `indicatif` crate (new dependency)
- `console` crate (transitive via indicatif, provides isatty)
