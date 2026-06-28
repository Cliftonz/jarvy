---
title: "CLI Reference - Jarvy"
description: "Complete reference for all Jarvy CLI commands, flags, and exit codes."
tags:
  - reference

---

# CLI Reference

## Core Commands

### `jarvy setup`

Provision the environment from a `jarvy.toml` configuration file.

```bash
jarvy setup                          # Default: ./jarvy.toml
jarvy setup --file path/to/config    # Custom config path
jarvy setup --from https://...       # Fetch config from URL
jarvy setup --dry-run                # Show what would happen
jarvy setup --plan                   # Alias for --dry-run
jarvy setup --role backend           # Override role for this run
jarvy setup --no-hooks               # Skip all hook execution
jarvy setup --ci                     # Force CI mode (non-interactive)
jarvy setup --jobs 8                 # Parallel jobs (default: 4)
jarvy setup --sequential             # Force sequential installation
```

| Flag | Description |
|------|-------------|
| `-f, --file` | Path to config file (default: `./jarvy.toml`) |
| `--from <URL>` | Fetch config from URL |
| `--role <ROLE>` | Override role assignment |
| `--dry-run / --plan` | Preview without executing |
| `--no-hooks` | Skip hook execution |
| `--ci` | Force CI mode |
| `--no-ci` | Force interactive mode in CI |
| `-j, --jobs` | Parallel install jobs (default: 4) |
| `--sequential` | Equivalent to `--jobs 1` |
| `--ignore-missing-deps` | Suppress dependency warnings |
| `-q, --quiet` | Suppress all output except errors |
| `-v, --verbose` | Verbose output (-vv for debug, -vvv for trace) |
| `--profile` | Enable performance profiling |

### `jarvy init`

Create a `jarvy.toml` interactively or from a template.

```bash
jarvy init                           # Interactive prompts
jarvy init --template react          # Use a template
jarvy init --template go-api --non-interactive
jarvy init --stdout                  # Print to stdout instead of file
```

### `jarvy doctor`

Diagnose environment health.

```bash
jarvy doctor                         # Check all configured tools
jarvy doctor --tools node,docker     # Check specific tools
jarvy doctor --extended              # Full system dashboard
jarvy doctor --format json           # Machine-readable output
jarvy doctor --report report.md      # Export as markdown
```

### `jarvy diff`

Preview changes before running setup (what's missing, outdated, or satisfied).

```bash
jarvy diff                           # Show all tools
jarvy diff --changes-only            # Only show needed changes
jarvy diff --format json
```

---

## Discovery Commands

### `jarvy search`

Search available tools.

```bash
jarvy search docker                  # Fuzzy search
jarvy search --all                   # List all 174+ tools
jarvy search --format json
```

### `jarvy explain`

Get detailed information about a specific tool.

```bash
jarvy explain docker                 # Tool details, platforms, deps
jarvy explain node --file jarvy.toml # Include config context (roles, version)
jarvy explain kubectl --format json
```

### `jarvy tools`

List all supported tools or output the tool index.

```bash
jarvy tools                          # List all tools
jarvy tools --index                  # Full tool index
jarvy tools --default-hooks          # Tools with built-in hooks
jarvy tools --index --format json    # JSON tool index
```

---

## Configuration Commands

### `jarvy validate`

Validate a `jarvy.toml` configuration file.

```bash
jarvy validate                       # Validate ./jarvy.toml
jarvy validate --from https://...    # Validate remote config
jarvy validate --strict              # Treat warnings as errors
```

### `jarvy migrate`

Check for deprecated patterns and suggest fixes.

```bash
jarvy migrate                        # Dry-run report
jarvy migrate --apply                # Apply migrations
jarvy migrate --format json
```

### `jarvy export`

Generate a `jarvy.toml` from currently installed tools.

```bash
jarvy export                         # Detect and export
jarvy export --all                   # Include all detected tools
jarvy export --tools node,docker     # Specific tools only
jarvy export --format json
```

### `jarvy configure`

Generate a default `jarvy.toml` in the current directory.

```bash
jarvy configure
```

---

## Team Commands

### `jarvy roles`

Manage role-based configurations.

```bash
jarvy roles list                     # List available roles
jarvy roles list -v                  # Verbose with tool counts
jarvy roles show frontend            # Show role details
jarvy roles show frontend --resolved # Include inherited tools
jarvy roles show frontend --inheritance  # Show inheritance chain
jarvy roles diff frontend backend    # Compare two roles
```

### `jarvy team`

Manage team configuration sources.

```bash
jarvy team list                      # List configured sources
jarvy team add <url>                 # Add a team config source
jarvy team remove <name>             # Remove a source
```

---

## Environment Commands

### `jarvy env`

Manage environment variables from config.

```bash
jarvy env                            # Apply all env config
jarvy env --dotenv                   # Generate .env file only
jarvy env --shell                    # Update shell rc file only
jarvy env --dry-run                  # Preview changes
jarvy env --export                   # Output export statements
```

### `jarvy drift`

Detect configuration drift.

```bash
jarvy drift check                    # Detect drift (exit 1 if found)
jarvy drift check --format json      # JSON output for CI
jarvy drift status                   # Show baseline state
jarvy drift accept                   # Accept current state
jarvy drift accept --tools node      # Accept specific tools
jarvy drift fix                      # Remediate issues
jarvy drift fix --dry-run            # Preview fixes
```

---

## Security Commands

### `jarvy audit`

Run available security scanners and produce a unified report.

```bash
jarvy audit                          # Run all available scanners
jarvy audit --tool betterleaks       # Run specific scanner
jarvy audit --format json
```

Supported scanners: betterleaks, gitleaks, trufflehog, trivy, grype, semgrep, checkov, tfsec.

---

## CI/CD Commands

### `jarvy ci-config`

Generate CI configuration files.

```bash
jarvy ci-config github               # GitHub Actions workflow
jarvy ci-config gitlab               # GitLab CI config
jarvy ci-config circleci             # CircleCI config
jarvy ci-config --dry-run            # Preview without writing
```

### `jarvy ci-info`

Show detected CI environment information.

```bash
jarvy ci-info
```

---

## Maintenance Commands

### `jarvy update`

Check for and install Jarvy updates.

```bash
jarvy update                         # Check and install latest
jarvy update check                   # Check without installing
jarvy update --version 1.2.3         # Install specific version
jarvy update --channel beta          # Use beta channel
jarvy update --rollback              # Rollback to previous version
jarvy update history                 # Show update history
```

### `jarvy upgrade`

Upgrade configured tools to latest versions.

```bash
jarvy upgrade                        # Upgrade all tools
jarvy upgrade --tools node,docker    # Upgrade specific tools
jarvy upgrade --dry-run              # Preview upgrades
```

### `jarvy diagnose`

Deep diagnosis for a specific tool.

```bash
jarvy diagnose docker                # Full diagnosis
jarvy diagnose node --fix            # Attempt auto-fix
```

---

## Utility Commands

### `jarvy schema`

Output the JSON Schema for `jarvy.toml`.

```bash
jarvy schema                         # Print to stdout
jarvy schema --output schema.json    # Write to file
```

Use this for editor autocomplete. In VS Code, add to `.vscode/settings.json`:

```json
{
  "json.schemas": [{
    "fileMatch": ["jarvy.toml"],
    "url": "./schema.json"
  }]
}
```

### `jarvy completions`

Generate shell completions.

```bash
jarvy completions bash               # Bash completions
jarvy completions zsh                # Zsh completions
jarvy completions fish               # Fish completions
jarvy completions --instructions     # Show install instructions
```

### `jarvy logs`

View and manage log files.

```bash
jarvy logs view                      # View recent logs
jarvy logs view --lines 50           # Last 50 lines
jarvy logs stats                     # Log statistics
jarvy logs clean                     # Remove old logs
```

### `jarvy ticket`

Generate debug tickets for support.

```bash
jarvy ticket create                  # Generate diagnostic bundle
jarvy ticket create --tool docker    # Tool-specific ticket
jarvy ticket list                    # List existing tickets
```

### `jarvy mcp`

Start the MCP server for AI agent integration. See the [MCP Server Guide](mcp-server.md).

```bash
jarvy mcp
```

---

## AI Integration Commands

### `jarvy ai-hooks`

Manage AI agent guardrail hooks (Claude Code, Cursor, Codex, Windsurf, Cline, Continue). See the [AI Hooks guide](ai-hooks.md).

```bash
jarvy ai-hooks list                  # show what's configured
jarvy ai-hooks list --library        # show built-in library hooks
jarvy ai-hooks apply                 # write hooks to every targeted agent
jarvy ai-hooks check                 # diff desired vs. on-disk (exit 1 if drift)
jarvy ai-hooks remove                # strip jarvy-managed entries
```

### `jarvy mcp-register`

Register the Jarvy MCP server (and optional library/custom servers) with terminal AI agents. See the [MCP Registration guide](mcp-registration.md).

```bash
jarvy mcp-register list              # show what's configured
jarvy mcp-register apply             # write registrations to every targeted agent
jarvy mcp-register check             # diff desired vs. on-disk
jarvy mcp-register remove            # strip jarvy-managed entries
```

### `jarvy skills`

Install AI agent skills from a library manifest URL across detected agents. See the [Skills guide](skills.md). PRD-049 + PRD-054.

```bash
jarvy skills install                 # install every skill from [skills.install]
jarvy skills install --name <skill>  # install one named skill
jarvy skills list                    # show configured skills + per-agent status
jarvy skills status                  # drift summary
jarvy skills agents                  # which AI agents are detected on disk
```

### `jarvy hooks`

Manage git pre-commit framework hooks. See the [Git Hooks guide](git-hooks.md). PRD-048.

```bash
jarvy hooks install                  # install framework into .git/hooks/
jarvy hooks update                   # pre-commit autoupdate + reinstall
jarvy hooks status                   # framework + installed?  + hook count
jarvy hooks list                     # configured hooks from .pre-commit-config.yaml
jarvy hooks run                      # run hooks against staged changes
jarvy hooks run --all-files          # run against entire tree
jarvy hooks run --hook black         # run a single hook by id
jarvy hooks uninstall                # pre-commit uninstall
```

---

## Exit Codes

| Code | Name | Meaning |
|------|------|---------|
| 0 | `EXIT_SUCCESS` | Command completed successfully |
| 2 | `CONFIG_ERROR` | jarvy.toml is missing or malformed |
| 3 | `PREREQ_MISSING` | Required package manager not found |
| 4 | `NETWORK_TIMEOUT` | Network or proxy failure |
| 5 | `PERMISSION_REQUIRED` | Needs elevated privileges (sudo) |
| 6 | `INCOMPATIBLE_OS_ARCH` | Unsupported OS or architecture |
| 7 | `HOOK_FAILED` | A hook script failed |

See [Error Codes](error-codes.md) for detailed remediation steps.

---

## Global Behavior

- **CI detection**: Jarvy auto-detects CI environments (`CI=true`) and switches to non-interactive mode
- **Config discovery**: Looks for `jarvy.toml` in the current directory by default
- **Idempotent**: Safe to run repeatedly — skips tools that are already installed and satisfied
- **Platform-aware**: Automatically selects the right package manager for each OS
