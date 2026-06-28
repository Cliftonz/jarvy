---
title: "CLI reference (auto-generated) — Jarvy"
description: "Complete jarvy command-line reference, generated from the binary's --help output. Always reflects the latest version."
tags:
  - reference
---

# CLI reference

!!! info "Auto-generated"
    This page is generated from `jarvy --help` by `scripts/gen-docs.sh`. To
    update it, run that script after a `cargo build`. Anything you write
    here by hand will be overwritten on the next regeneration.

## `jarvy`

```text
Jarvy helps you set up and verify your computer based on a jarvy.toml configuration.

USAGE:
    jarvy <COMMAND> [OPTIONS]

EXAMPLES:
    jarvy --help
    jarvy configure
    jarvy setup --file ./jarvy.toml
    jarvy get --format json --output report.json

Run without a subcommand to use the interactive menu.

Usage: jarvy [COMMAND]

Commands:
  setup         Set up the environment based on the configuration file
  bootstrap     Perform a minimal machine bootstrap (base requirements only, no dev tooling)
  configure     Generate a default jarvy.toml configuration in the current directory
  get           Display configured tools vs what is actually installed
  tools         List all supported tools or output the tool index
  env           Manage environment variables from jarvy.toml
  ci-config     Generate CI configuration files for various providers
  ci-info       Show detected CI environment information
  services      Manage project services (docker-compose, tilt)
  doctor        Diagnose environment issues, check tool health, and verify PATH
  diff          Preview changes before running setup (dry-run)
  export        Generate jarvy.toml from currently installed tools
  upgrade       Upgrade tools to their latest versions
  init          Create a new jarvy.toml configuration file interactively
  search        Search available tools that Jarvy can install
  validate      Validate a jarvy.toml configuration file
  completions   Generate shell completions
  templates     Browse and use pre-built configuration templates
  registry      Sync + inspect the remote tool registry configured in ~/.jarvy/config.toml [registry]
  telemetry     Manage telemetry settings (OTEL endpoint, signals)
  mcp           Start the MCP (Model Context Protocol) server for LLM integration
  diagnose      Deep diagnosis for a specific tool - check installation, dependencies, and health
  team          Manage team configuration sources for shared configs
  roles         Manage role-based configurations (list, show, diff)
  lock          Manage version lock files for reproducible environments
  config        Manage configuration inheritance and remote configs
  quickstart    Guided quickstart experience for new users
  update        Check for and install Jarvy updates
  drift         Detect configuration drift in the environment
  logs          View and manage log files
  ticket        Generate debug tickets for support
  shell-init    Output shell initialization snippet for RC files. Add `eval "$(jarvy shell-init)"` to your .bashrc/.zshrc
  ensure        Ensure base tools are installed (lightweight check for shell startup). Reads tool list from [shell_init] in ~/.jarvy/config.toml
  explain       Get detailed information about a specific tool
  audit         Run security scanners and produce a unified audit report
  migrate       Check jarvy.toml for deprecated patterns and suggest migrations
  schema        Output the JSON Schema for jarvy.toml (for editor autocomplete)
  ai-hooks      Manage AI agent hooks (Claude Code / Cursor / Codex / Windsurf / Cline / Continue)
  mcp-register  Register the Jarvy MCP server with terminal AI agents
  hooks         Manage git hook frameworks (pre-commit, husky, lefthook)
  skills        Install and manage AI agent skills from library_sources (PRD-049 + PRD-054)
  help          Print this message or the help of the given subcommand(s)

Options:
  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```

## Subcommands

### `jarvy setup`

```text
Set up the environment based on the configuration file

Usage: jarvy setup [OPTIONS]

Options:
  -f, --file <FILE>            Path to the configuration file [default: ./jarvy.toml]
      --from <URL>             Fetch configuration from a URL (e.g., GitHub raw URL, gist, HTTP endpoint)
      --role <ROLE>            Override role assignment for this run (temporary, doesn't modify config)
      --no-hooks               Skip all hook execution
      --dry-run                Show what would happen without executing (dry run mode)
      --ci                     Force CI mode (non-interactive, auto-answer prompts)
      --no-ci                  Force interactive mode even in CI environments
  -j, --jobs <JOBS>            Number of parallel jobs for user-space package installations (npm, pip, cargo, go, custom installers). Default: 4. Set to 1 for sequential installation [default: 4]
      --sequential             Force sequential installation (equivalent to --jobs 1). Useful for deterministic output
      --ignore-missing-deps    Ignore missing dependency warnings (advanced use). Normally, jarvy warns when installing tools whose dependencies are missing. Use this flag to suppress those warnings (e.g., if dependencies are pre-installed elsewhere)
      --header <HEADER>        Add custom HTTP header for authenticated config fetching (can be repeated) Example: --header "Authorization: token ghp_xxxx" --header "X-Custom: value"
  -q, --quiet                  Suppress all output except errors
  -v, --verbose...             Verbose output (use -v for warnings, -vv for debug, -vvv for trace)
      --profile                Enable performance profiling
      --profile-output <FILE>  Write profile results to file (JSON)
      --log-format <FORMAT>    Log output format: text (default), json
      --log-file <FILE>        Write logs to file instead of stderr
      --debug-filter <MODULE>  Filter debug logs to specific modules (e.g., jarvy::tools::docker)
  -h, --help                   Print help
```

### `jarvy bootstrap`

```text
Perform a minimal machine bootstrap (base requirements only, no dev tooling)

Usage: jarvy bootstrap

Options:
  -h, --help  Print help
```

### `jarvy configure`

```text
Generate a default jarvy.toml configuration in the current directory

Usage: jarvy configure

Options:
  -h, --help  Print help
```

### `jarvy get`

```text
Display configured tools vs what is actually installed

Usage: jarvy get [OPTIONS]

Options:
  -f, --file <FILE>             Path to the configuration file [default: ./jarvy.toml]
  -F, --format <OUTPUT_FORMAT>  Output format: json, yaml, toml, pretty [default: pretty] [possible values: json, yaml, toml, pretty]
  -o, --output <OUTPUT>         Optional file to write output to; prints to stdout if omitted
  -h, --help                    Print help
```

### `jarvy tools`

```text
List all supported tools or output the tool index

Usage: jarvy tools [OPTIONS]

Options:
      --index                   Output the full tool index as JSON
      --default-hooks           List tools with built-in default hooks
      --request <TOOL>          Generate a pre-filled GitHub issue URL and scaffold snippet for requesting support for an unsupported tool
      --open                    With --request, open the pre-filled GitHub issue in the default browser instead of just printing the URL
  -F, --format <OUTPUT_FORMAT>  Output format: json, yaml, toml, pretty (for --index) [default: pretty] [possible values: json, yaml, toml, pretty]
  -o, --output <OUTPUT>         Optional file to write output to; prints to stdout if omitted
  -h, --help                    Print help
```

### `jarvy env`

```text
Manage environment variables from jarvy.toml

Usage: jarvy env [OPTIONS]

Options:
  -f, --file <FILE>              Path to the configuration file [default: ./jarvy.toml]
      --dotenv                   Generate .env file only
      --shell                    Update shell rc file only
      --dry-run                  Show what would happen without making changes
      --export                   Output for shell eval (export statements)
      --shell-type <SHELL_TYPE>  Shell type to use (bash, zsh, fish). Auto-detected if not specified
      --force                    Force overwrite of existing .env file (even if not created by Jarvy)
  -h, --help                     Print help
```

### `jarvy ci-config`

```text
Generate CI configuration files for various providers

Usage: jarvy ci-config [OPTIONS] <PROVIDER>

Arguments:
  <PROVIDER>  CI provider to generate config for (github, gitlab, circleci, azure, bitbucket)

Options:
  -o, --output <OUTPUT>  Output directory (defaults to current directory) [default: .]
      --dry-run          Show the config without writing to file
  -h, --help             Print help
```

### `jarvy ci-info`

```text
Show detected CI environment information

Usage: jarvy ci-info

Options:
  -h, --help  Print help
```

### `jarvy services`

```text
Manage project services (docker-compose, tilt)

Usage: jarvy services [OPTIONS] <COMMAND>

Commands:
  start    Start project services
  stop     Stop project services
  status   Show service status
  restart  Restart project services
  help     Print this message or the help of the given subcommand(s)

Options:
  -f, --file <FILE>  Path to the configuration file [default: ./jarvy.toml]
  -h, --help         Print help
```

### `jarvy doctor`

```text
Diagnose environment issues, check tool health, and verify PATH

Usage: jarvy doctor [OPTIONS]

Options:
  -f, --file <FILE>             Path to the configuration file (optional)
      --tools <TOOLS>           Only check specific tools (comma-separated)
  -F, --format <OUTPUT_FORMAT>  Output format: json, pretty [default: pretty]
      --extended                Show extended health dashboard with system metrics
      --report <REPORT>         Export diagnostic report as markdown
  -h, --help                    Print help
```

### `jarvy diff`

```text
Preview changes before running setup (dry-run)

Usage: jarvy diff [OPTIONS]

Options:
  -f, --file <FILE>             Path to the configuration file [default: ./jarvy.toml]
      --changes-only            Only show changes (hide satisfied tools)
  -F, --format <OUTPUT_FORMAT>  Output format: json, pretty [default: pretty]
  -h, --help                    Print help
```

### `jarvy export`

```text
Generate jarvy.toml from currently installed tools

Usage: jarvy export [OPTIONS]

Options:
      --tools <TOOLS>           Only include specific tools (comma-separated)
      --all                     Include all detected tools
  -v, --verbose                 Show verbose output (include paths)
  -F, --format <OUTPUT_FORMAT>  Output format: toml, json [default: toml]
  -o, --output <OUTPUT>         Output file (stdout if not specified)
  -h, --help                    Print help
```

### `jarvy upgrade`

```text
Upgrade tools to their latest versions

Usage: jarvy upgrade [OPTIONS]

Options:
  -f, --file <FILE>             Path to the configuration file (optional)
      --tools <TOOLS>           Only upgrade specific tools (comma-separated or tool@version)
      --dry-run                 Show what would be upgraded without making changes
      --force                   Force upgrade even if already at required version
  -F, --format <OUTPUT_FORMAT>  Output format: json, pretty [default: pretty]
  -h, --help                    Print help
```

### `jarvy init`

```text
Create a new jarvy.toml configuration file interactively

Usage: jarvy init [OPTIONS]

Options:
  -t, --template <TEMPLATE>  Use a predefined template (react, vue, go-api, rust-cli, etc.)
      --non-interactive      Run without interactive prompts (requires --template)
      --stdout               Output to stdout instead of file
  -o, --output <OUTPUT>      Output file path (default: jarvy.toml)
  -h, --help                 Print help
```

### `jarvy search`

```text
Search available tools that Jarvy can install

Usage: jarvy search [OPTIONS] [QUERY]

Arguments:
  [QUERY]  Search query (tool name or partial match)

Options:
      --all                     Show all available tools
  -F, --format <OUTPUT_FORMAT>  Output format: json, pretty [default: pretty]
  -h, --help                    Print help
```

### `jarvy validate`

```text
Validate a jarvy.toml configuration file

Usage: jarvy validate [OPTIONS]

Options:
  -f, --file <FILE>             Path to the configuration file [default: ./jarvy.toml]
      --from <URL>              Fetch configuration from a URL and validate it (e.g., GitHub raw URL, gist)
      --strict                  Treat warnings as errors
      --header <HEADER>         Add custom HTTP header for authenticated config fetching (can be repeated) Example: --header "Authorization: token ghp_xxxx"
  -F, --format <OUTPUT_FORMAT>  Output format: json, pretty [default: pretty]
  -h, --help                    Print help
```

### `jarvy completions`

```text
Generate shell completions

Usage: jarvy completions [OPTIONS] <SHELL>

Arguments:
  <SHELL>  Shell to generate completions for (bash, zsh, fish, powershell, elvish)

Options:
      --instructions  Show installation instructions
  -h, --help          Print help
```

### `jarvy templates`

```text
Browse and use pre-built configuration templates

Usage: jarvy templates <COMMAND>

Commands:
  list  List all available templates
  show  Show details of a specific template
  use   Use a template to create jarvy.toml
  help  Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```

### `jarvy registry`

```text
Sync + inspect the remote tool registry configured in ~/.jarvy/config.toml [registry]

Usage: jarvy registry <COMMAND>

Commands:
  sync    Fetch the remote registry: verify signature, sha-verify each tool TOML, and cache under ~/.jarvy/tools.d/.remote/. The next `jarvy setup` / `jarvy validate` run picks up the synced tools via the plugin loader
  status  Show the last sync's metadata (URL, count, timestamp, signature-verified flag)
  clear   Clear the local registry cache. Synced tools disappear on next startup until you run `jarvy registry sync` again
  help    Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```

### `jarvy telemetry`

```text
Manage telemetry settings (OTEL endpoint, signals)

Usage: jarvy telemetry <COMMAND>

Commands:
  status        Show current telemetry configuration
  enable        Enable telemetry
  disable       Disable telemetry
  set-endpoint  Set OTLP endpoint URL
  test          Test telemetry connectivity
  preview       Preview what telemetry would be sent
  help          Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```

### `jarvy mcp`

```text
Start the MCP (Model Context Protocol) server for LLM integration

Usage: jarvy mcp [OPTIONS]

Options:
  -c, --config <CONFIG>  Path to MCP configuration file (defaults to ~/.jarvy/mcp-config.toml)
  -h, --help             Print help
```

### `jarvy diagnose`

```text
Deep diagnosis for a specific tool - check installation, dependencies, and health

Usage: jarvy diagnose [OPTIONS] <TOOL>

Arguments:
  <TOOL>  Tool to diagnose (e.g., 'docker', 'node', 'git')

Options:
      --fix                     Attempt to automatically fix detected issues
      --export                  Export diagnostic bundle to a file
      --scope <SCOPE>           Scope for export: tools, network, all (comma-separated) [default: all]
  -F, --format <OUTPUT_FORMAT>  Output format: json, pretty [default: pretty]
  -h, --help                    Print help
```

### `jarvy team`

```text
Manage team configuration sources for shared configs

Usage: jarvy team <COMMAND>

Commands:
  add     Add a team configuration source
  list    List registered team sources
  browse  Browse available configs from a source
  sync    Sync config index from a source
  remove  Remove a team source
  init    Initialize project with a team config
  help    Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```

### `jarvy roles`

```text
Manage role-based configurations (list, show, diff)

Usage: jarvy roles [OPTIONS] <COMMAND>

Commands:
  list  List all available roles
  show  Show details for a specific role
  diff  Compare two roles
  help  Print this message or the help of the given subcommand(s)

Options:
  -f, --file <FILE>  Path to the configuration file [default: ./jarvy.toml]
  -h, --help         Print help
```

### `jarvy lock`

```text
Manage version lock files for reproducible environments

Usage: jarvy lock <COMMAND>

Commands:
  generate  Generate a lock file from current environment
  status    Show lock file status (compare with installed versions)
  verify    Verify installed tools match lock file
  help      Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```

### `jarvy config`

```text
Manage configuration inheritance and remote configs

Usage: jarvy config <COMMAND>

Commands:
  show     Show resolved configuration (with inheritance applied)
  refresh  Refresh cached remote configs
  help     Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```

### `jarvy quickstart`

```text
Guided quickstart experience for new users

Usage: jarvy quickstart [OPTIONS]

Options:
      --non-interactive  Run without interactive prompts
      --skip-check       Skip system check step
  -h, --help             Print help
```

### `jarvy update`

```text
Check for and install Jarvy updates

Usage: jarvy update [OPTIONS] [COMMAND]

Commands:
  check    Check for available updates
  history  Show update history
  config   Show update configuration
  enable   Enable auto-updates
  disable  Disable auto-updates
  help     Print this message or the help of the given subcommand(s)

Options:
      --version <VERSION>  Install specific version
      --channel <CHANNEL>  Use specific release channel (stable, beta, nightly)
      --method <METHOD>    Override installation method (homebrew, cargo, apt, dnf, winget, chocolatey, scoop, binary)
      --rollback           Rollback to previous version
      --allow-unsigned     Skip Sigstore signature verification (DANGEROUS — only when cosign is unavailable and you accept supply-chain risk)
  -h, --help               Print help
```

### `jarvy drift`

```text
Detect configuration drift in the environment

Usage: jarvy drift [OPTIONS] <COMMAND>

Commands:
  check   Check for configuration drift
  status  Show current state baseline
  accept  Accept current state as new baseline
  fix     Fix detected drift issues
  help    Print this message or the help of the given subcommand(s)

Options:
  -f, --file <FILE>  Path to the configuration file [default: ./jarvy.toml]
  -h, --help         Print help
```

### `jarvy logs`

```text
View and manage log files

Usage: jarvy logs <COMMAND>

Commands:
  view    View recent log entries
  stats   Show log statistics
  clean   Clean old log files
  config  Show logging configuration
  help    Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```

### `jarvy ticket`

```text
Generate debug tickets for support

Usage: jarvy ticket <COMMAND>

Commands:
  create  Create a new debug ticket
  show    Show contents of a ticket
  list    List existing tickets
  clean   Clean old tickets
  help    Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```

### `jarvy shell-init`

```text
Output shell initialization snippet for RC files. Add `eval "$(jarvy shell-init)"` to your .bashrc/.zshrc

Usage: jarvy shell-init [OPTIONS]

Options:
      --shell <SHELL>  Shell type (bash, zsh, fish, sh, powershell). Auto-detected if not specified
  -h, --help           Print help
```

### `jarvy ensure`

```text
Ensure base tools are installed (lightweight check for shell startup). Reads tool list from [shell_init] in ~/.jarvy/config.toml

Usage: jarvy ensure [OPTIONS]

Options:
      --force       Force re-check, ignore stamp file
  -q, --quiet       Suppress all output
      --foreground  Run in foreground (override background default)
  -h, --help        Print help
```

### `jarvy explain`

```text
Get detailed information about a specific tool

Usage: jarvy explain [OPTIONS] <TOOL>

Arguments:
  <TOOL>  Tool to explain (e.g., 'docker', 'node', 'git')

Options:
  -f, --file <FILE>             Path to the configuration file (optional, for role/version context)
  -F, --format <OUTPUT_FORMAT>  Output format: json, pretty [default: pretty]
  -h, --help                    Print help
```

### `jarvy audit`

```text
Run security scanners and produce a unified audit report

Usage: jarvy audit [OPTIONS]

Options:
      --tool <TOOL>             Run only a specific scanner (betterleaks, gitleaks, trivy, etc.)
  -F, --format <OUTPUT_FORMAT>  Output format: json, pretty [default: pretty]
  -h, --help                    Print help
```

### `jarvy migrate`

```text
Check jarvy.toml for deprecated patterns and suggest migrations

Usage: jarvy migrate [OPTIONS]

Options:
  -f, --file <FILE>             Path to the configuration file [default: ./jarvy.toml]
      --apply                   Apply migrations (default is dry-run report only)
  -F, --format <OUTPUT_FORMAT>  Output format: json, pretty [default: pretty]
  -h, --help                    Print help
```

### `jarvy schema`

```text
Output the JSON Schema for jarvy.toml (for editor autocomplete)

Usage: jarvy schema [OPTIONS]

Options:
  -o, --output <OUTPUT>  Write to file instead of stdout
  -h, --help             Print help
```

### `jarvy ai-hooks`

```text
Manage AI agent hooks (Claude Code / Cursor / Codex / Windsurf / Cline / Continue)

Usage: jarvy ai-hooks [OPTIONS] <COMMAND>

Commands:
  list    List provisioned hooks or the built-in library
  apply   Write hook configs to every targeted AI agent
  check   Diff desired vs. on-disk state (exit 1 if drift)
  remove  Strip jarvy-managed entries from every targeted agent
  test    Inspect a single library hook (event, matcher, script bodies)
  help    Print this message or the help of the given subcommand(s)

Options:
  -f, --file <FILE>  Path to the configuration file [default: ./jarvy.toml]
  -h, --help         Print help
```

### `jarvy mcp-register`

```text
Register the Jarvy MCP server with terminal AI agents

Usage: jarvy mcp-register [OPTIONS] <COMMAND>

Commands:
  list    Show what's in jarvy.toml + agent → path mapping
  apply   Register the Jarvy MCP server with every targeted agent
  check   Diff desired vs. on-disk state (exit 1 if drift)
  remove  Strip jarvy-managed entries from every targeted agent
  help    Print this message or the help of the given subcommand(s)

Options:
  -f, --file <FILE>  Path to the configuration file [default: ./jarvy.toml]
  -h, --help         Print help
```

### `jarvy hooks`

```text
Manage git hook frameworks (pre-commit, husky, lefthook)

Usage: jarvy hooks [OPTIONS] <COMMAND>

Commands:
  install    Install the configured git hook framework into `.git/hooks/`
  update     Run `pre-commit autoupdate` then reinstall hooks
  status     Show framework + installation status
  list       List configured hooks from `.pre-commit-config.yaml`
  run        Run hooks once (defaults to changed files; `--all-files` for whole tree)
  uninstall  Remove jarvy-installed hooks (calls `pre-commit uninstall`)
  help       Print this message or the help of the given subcommand(s)

Options:
  -f, --file <FILE>  Path to the configuration file [default: ./jarvy.toml]
  -h, --help         Print help
```

### `jarvy skills`

```text
Install and manage AI agent skills from library_sources (PRD-049 + PRD-054)

Usage: jarvy skills [OPTIONS] <COMMAND>

Commands:
  install  Install every skill from `[skills.install]`, or a single named skill
  list     List skills declared in jarvy.toml + their installation status across agents
  status   Drift check: which configured skills are missing / out-of-version per agent
  agents   Show which AI agents are detected on disk
  help     Print this message or the help of the given subcommand(s)

Options:
  -f, --file <FILE>  Path to the configuration file [default: ./jarvy.toml]
  -h, --help         Print help
```

### `jarvy help`

```text
error: unrecognized subcommand '--help'

Usage: jarvy [COMMAND]

For more information, try '--help'.
```

