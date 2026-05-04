# PRD-051: Universal Structured Output (`--format json` on All Commands)

## Overview

Add `--format json` support to every CLI command so that AI agents, scripts, and CI pipelines can consume Jarvy output programmatically.

## Problem Statement

Many Jarvy commands already support `--format json` (get, tools, doctor, diff, search, export, upgrade, validate, explain, audit, migrate), but several do not:

- `jarvy services` (start/stop/status)
- `jarvy roles list/show/diff`
- `jarvy drift check/status/accept/fix`
- `jarvy logs view/stats/config`
- `jarvy ticket create/show/list`
- `jarvy ci-info`
- `jarvy diagnose`
- `jarvy completions` (output-only, may not apply)

AI agents calling Jarvy via MCP or shell need machine-readable output from every command. Human-only output forces agents to parse free-text, which is fragile.

## Evidence

- MCP server already returns JSON for tool queries, but CLI commands invoked by agents produce unparseable text
- CI pipelines parsing `jarvy drift check` output use fragile regex
- PRD-016 Non-Functional Requirement #1 states "All commands support `--format` flag" but this was not fully implemented

## Requirements

### Functional Requirements

1. Every command that produces output must accept `--format` with values: `pretty` (default), `json`
2. JSON output must be valid, parseable JSON (not mixed with log lines)
3. JSON output uses the `Outputable` trait pattern from `src/output/mod.rs`
4. Exit codes remain consistent regardless of format

### Non-Functional Requirements

1. No performance regression for human-readable (default) output
2. JSON schema for each command's output should be documented
3. `--format json` suppresses all stderr decorations (spinners, colors, progress)

## Non-Goals

- YAML/TOML output for commands that don't already support it
- Binary/protobuf output formats
- Streaming JSON (newline-delimited)

## Implementation

### Commands to Update

| Command | Current Output | Module |
|---------|---------------|--------|
| `services start/stop/status` | println! | `src/commands/services_cmd.rs` |
| `roles list/show/diff` | println! | `src/commands/roles_cmd.rs` |
| `drift check/status/fix` | println! | `src/commands/drift_cmd.rs` |
| `logs view/stats/config` | println! | `src/commands/logs_cmd.rs` |
| `ticket create/show/list` | println! | `src/commands/ticket_cmd.rs` |
| `ci-info` | println! | `src/commands/ci_cmd.rs` |
| `diagnose` | mixed | `src/commands/diagnose.rs` |

### Pattern

For each command:
1. Create a `#[derive(Serialize)]` result struct
2. Implement `Outputable` trait (provides `to_human()`, `to_json()`, `exit_code()`)
3. Add `--format` flag to CLI args
4. Use `result.render(format)` in the handler

### Testing

- Each command gets a test verifying JSON output is valid
- Integration tests: `jarvy <cmd> --format json | jq .` succeeds

## Effort Estimate

3-4 days (mostly mechanical, one struct + trait impl per command)

## Dependencies

- `src/output/mod.rs` — `Outputable` trait (already exists)
- `serde` / `serde_json` (already dependencies)
