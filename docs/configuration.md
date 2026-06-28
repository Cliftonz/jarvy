---
title: "Configuration Reference - Jarvy"
description: "Complete reference for jarvy.toml — tools, versions, hooks, environment variables, roles, drift detection, and more."
tags:
  - reference

---

# Configuration Reference

Jarvy reads a `jarvy.toml` file in your project root. This page documents every section and option.

## Minimal Example

```toml
[provisioner]
git = "latest"
node = "20"
docker = "latest"
```

Run `jarvy setup` and every developer gets the same tools.

## Tool Versions (`[provisioner]`)

The `[provisioner]` section lists tools to install with version requirements.

### Simple Format

```toml
[provisioner]
git = "latest"
node = "20"
python = ">=3.10"
```

### Detailed Format

```toml
[provisioner]
node = { version = "20", version_manager = true }
python = { version = "3.12", use_sudo = false }
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `version` | string | required | Version requirement (`"latest"`, `"20"`, `">=3.10"`, `"^1.0"`, `"~3.12"`, `"=1.2.3"`) |
| `version_manager` | bool | `true` | Use version manager (nvm for node, pyenv for python) if available |
| `use_sudo` | bool | auto | Override sudo usage for this tool |

### Version Operators

| Operator | Example | Meaning |
|----------|---------|---------|
| `latest` | `"latest"` | Any version is accepted |
| bare | `"20"` | Major version 20.x.x |
| `>=` | `">=3.10"` | 3.10 or higher |
| `^` | `"^1.0"` | Compatible with 1.0 (>=1.0.0, <2.0.0) |
| `~` | `"~3.12"` | Approximately 3.12 (>=3.12.0, <3.13.0) |
| `=` | `"=1.2.3"` | Exactly this version |

---

## Hooks (`[hooks]`)

Run shell scripts at specific points during setup.

### Global Hooks

```toml
[hooks]
pre_setup = "echo 'Starting setup...'"
post_setup = "echo 'Setup complete!'"
```

### Per-Tool Hooks

```toml
[hooks.node]
post_install = "npm install -g typescript eslint prettier"

[hooks.python]
post_install = "pip install --user black mypy"
```

### Hook Settings

```toml
[hooks.config]
shell = "/bin/bash"        # Shell for hook execution (default: $SHELL)
timeout = 300              # Timeout in seconds (default: 300)
continue_on_error = false  # Keep going if a hook fails
```

### Default Hooks

Many tools include built-in default hooks that run automatically (shell integration, completions, etc.). Your per-tool hooks override defaults.

```bash
# List tools with default hooks
jarvy tools --default-hooks
```

---

## Environment Variables (`[env]`)

### Variables

```toml
[env.vars]
NODE_ENV = "development"
EDITOR = "vim"
API_URL = { value = "http://localhost:3000", description = "Backend API" }
```

### Secrets

```toml
[env.secrets]
API_KEY = { env = "MY_API_KEY", required = true, description = "Backend API key" }
DB_PASSWORD = { from_file = "~/.secrets/db_pass" }
```

### Settings

```toml
[env.config]
shell = "zsh"              # Target shell for rc file updates
update_rc = false          # Update shell rc files
generate_dotenv = true     # Generate .env file
dotenv_path = ".env"       # Path for .env file
add_to_gitignore = false   # Add .env to .gitignore
backup_rc = true           # Backup rc files before modification
```

---

## Roles (`[roles]`)

Define tool sets for different team members. See the [Roles Guide](roles.md) for details.

```toml
role = "frontend"

[roles.base]
description = "Base development tools"
tools = ["git", "docker"]

[roles.frontend]
extends = "base"
description = "Frontend development"
tools = ["node", "bun"]

[roles.frontend.tools]
node = "20"
bun = "latest"
```

---

## Services (`[services]`)

Manage Docker Compose and Tilt services.

```toml
[services]
enabled = true
auto_start = true          # Start services during jarvy setup
compose_file = "docker-compose.yml"
tilt_file = "Tiltfile"
start_in_ci = false        # Don't auto-start in CI
```

---

## Network / Proxy (`[network]`)

Configure HTTP/HTTPS/SOCKS proxies for corporate environments.

```toml
[network]
https_proxy = "http://proxy.corp.com:8080"
no_proxy = ["localhost", "127.0.0.1", ".corp.com"]

[network.auth]
username = "jdoe"
password = { env = "PROXY_PASSWORD" }

[network.tls]
ca_bundle = "/etc/ssl/certs/corporate-ca.crt"

# Per-tool overrides
[network.overrides.git]
https_proxy = "http://git-proxy.corp.com:8888"
```

---

## Git Configuration (`[git]`)

Automate Git settings across the team.

```toml
[git]
user_name = "Jane Doe"
user_email = { env = "GIT_EMAIL", default = "jane@example.com" }

# Commit signing
signing = true
signing_key = "~/.ssh/id_ed25519.pub"
signing_format = "ssh"         # "ssh" or "gpg"

# Defaults
default_branch = "main"
pull_rebase = true
auto_stash = true
push_autosetup = true
editor = "vim"

# Line endings
autocrlf = "input"
eol = "lf"

# Scope
scope = "global"               # "global" or "local"

# Aliases
[git.aliases]
co = "checkout"
br = "branch"
ci = "commit"
st = "status"
lg = "log --oneline --graph --decorate"
```

---

## Language Packages

Six ecosystems supported: `[npm]`, `[pip]`, `[cargo]`, `[nuget]`, `[gem]`, `[go]`. Each installs into the language's standard global location. Full surface in [packages](packages.md).

### npm

```toml
[npm]
typescript = "^5.0"
eslint = "latest"
package_manager = "pnpm"       # Auto-detected from lock file if not set
from_lockfile = false          # Install from package-lock.json instead
```

### pip

```toml
[pip]
pytest = ">=7.0"
black = "latest"
venv = ".venv"                 # Virtual environment path
create_venv = true             # Auto-create venv if missing
from_lockfile = false          # Install from requirements.txt instead
```

### cargo

```toml
[cargo]
cargo-watch = "latest"
cargo-nextest = "0.9"
locked = true                  # Use --locked flag
```

### nuget (.NET global tools)

```toml
[nuget]
dotnet-ef = "latest"
csharpier = "0.30.0"
```

Installs via `dotnet tool update -g <name>`. Project-level `<PackageReference>` deps are NOT managed here.

### gem (Ruby)

```toml
[gem]
bundler = "latest"
rubocop = "1.60.0"
```

Installs via `gem install --no-document <name>` against the active ruby.

### go (Go binaries)

```toml
[go]
"github.com/golangci/golangci-lint/cmd/golangci-lint" = "latest"
"github.com/cosmtrek/air" = "v1.49.0"
```

Module paths must be quoted in TOML; version is mandatory (`"latest"` for floating).

### Trust gate

Remote-fetched configs (`jarvy setup --from <url>`) cannot install language packages unless the source config sets `[packages] allow_remote = true`. Mirrors the trust pattern documented under [library registry](library-registry.md).

---

## AI Hooks (`[ai_hooks]`)

Distribute guardrail hooks across Claude Code, Cursor, Codex, Windsurf, Cline, and Continue from a single config. See [ai-hooks](ai-hooks.md) for the full surface.

```toml
[ai_hooks]
agents = ["claude-code", "cursor", "codex"]
scope = "user"                              # user | project
allow_custom_commands = false               # gate raw command = "..." entries

# Built-in library hook (always allowed):
[[ai_hooks.hook]]
use = "block-rm-rf"

# Third-party library reference (PRD-054):
[[ai_hooks.library_sources]]
url = "https://cdn.myorg.com/jarvy/manifest.json"

[[ai_hooks.hook]]
use = "myorg/no-prod-deploys"               # resolves from library_sources
```

Built-in `LIBRARY` hooks (`block-rm-rf`, `block-force-push`, ...) take precedence over library items with the same name.

---

## MCP Registration (`[mcp_register]`)

Auto-registers the Jarvy MCP server (and optional custom or library servers) with each developer's AI agents. See [mcp-registration](mcp-registration.md).

```toml
[mcp_register]
agents = ["claude-code", "cursor"]
scope = "user"                              # user | project (per agent support)
allow_custom_servers = false                # gate raw [[mcp_register.server]] entries

# Override the built-in Jarvy server entry (optional):
[mcp_register.jarvy]
command = "/usr/local/bin/jarvy"
args = ["mcp"]

# Inline custom server (subject to allow_custom_servers + local-origin):
[[mcp_register.server]]
name = "my-stdio-server"
command = "my-mcp-server"
args = ["--workspace", "."]

# Or reference a library server (PRD-054):
[[mcp_register.library_sources]]
url = "https://cdn.myorg.com/jarvy/manifest.json"

[[mcp_register.server]]
use = "myorg-tickets"                       # spec fields override library defaults
env = { LINEAR_API_KEY = "${LINEAR_API_KEY}" }
```

---

## Git Hooks (`[git_hooks]`)

Install pre-commit framework hooks during `jarvy setup`. See [git-hooks](git-hooks.md).

```toml
[git_hooks]
enabled = true                              # default true; block presence is the opt-in
framework = "pre-commit"                    # pre-commit (today) | husky / lefthook (stubbed)
auto_install = true                         # install during jarvy setup (default true)
auto_update = false                         # run pre-commit autoupdate after install
run_after_install = false                   # run hooks once against the whole tree
allow_remote = false                        # remote-config trust gate (default false)

[git_hooks.pre_commit]
version = "3.6.0"                           # pin framework version
config = ".pre-commit-config.yaml"          # path to framework config (default)
install_hooks = true                        # --install-hooks (warm envs eagerly)
```

Auto-detects framework from `.pre-commit-config.yaml` / `.husky/` / `lefthook.yml` when `framework` is unset. Husky / lefthook are recognized but their handlers are stubbed today.

---

## AI Agent Skills (`[skills]`)

Install SKILL.md skills across Claude Code, Cursor, Codex, etc. from a library manifest. See [skills](skills.md).

```toml
[skills]
auto_install = true                         # install during jarvy setup (default)
agents = ["claude-code", "cursor"]          # empty = auto-detect every agent on disk

[[skills.library_sources]]
url = "https://cdn.myorg.com/jarvy/manifest.json"

[skills.install]
myorg-code-review = "2.1.0"
myorg-debug-checklist = { version = "1.0.0", agents = ["claude-code"] }
```

Skills are sha256-verified against the manifest at install time. v1 skips skills.sh API integration, companion files, update/remove subcommands — tracked under PRD-049 phase 2.

---

## Library Sources (PRD-054)

A shared mechanism that lets three consumer blocks (`[ai_hooks]`, `[mcp_register]`, `[skills]`) pull reusable items from any HTTPS-hosted manifest. One manifest can publish all three kinds — see [library-registry](library-registry.md) for the spec.

```toml
[[ai_hooks.library_sources]]
url = "https://cdn.myorg.com/jarvy/manifest.json"
require_signature = true                    # default; cosign verification (enforcement phase 5)
identity_regexp = "^https://github\\.com/myorg/jarvy-library/.+$"
oidc_issuer = "https://token.actions.githubusercontent.com"
refresh_interval_secs = 86400               # disk-cache TTL (default 24h)
```

**Trust gate uniform across all three consumers**: remote-fetched configs (`jarvy setup --from <url>`) CANNOT declare `library_sources`. No override flag — teams copy URLs into each developer's local `~/.jarvy/config.toml` instead.

---

## Drift Detection (`[drift]`)

Detect when a developer's environment has changed from the expected configuration.

```toml
[drift]
enabled = true
check_on_run = false           # Check on every jarvy command
track_files = [".vscode/settings.json", "package.json"]
version_policy = "minor"       # major, minor, patch, exact
ignore_tools = ["vim", "neovim"]
allow_upgrades = true          # Only flag downgrades
```

---

## Workspace (`[workspace]`)

For monorepos with multiple sub-projects.

```toml
[workspace]
members = ["services/api", "services/web", "libs/shared"]
inherit = ["provisioner", "hooks", "env"]
```

Each member directory can have its own `jarvy.toml` that inherits from the root.

---

## Privileges (`[privileges]`)

Control sudo usage.

```toml
[privileges]
use_sudo = true

[privileges.per_os]
linux = true
macos = false
windows = false
```

---

## Custom Commands (`[commands]`)

Override the interactive menu defaults.

```toml
[commands]
run = "npm start"
test = "npm test"
setup = "jarvy setup"
```

---

## Config Inheritance (`extends`)

Inherit from a remote or local config.

```toml
extends = "https://raw.githubusercontent.com/org/configs/main/base.toml"
```

Or from a local file:

```toml
extends = "../shared/base-jarvy.toml"
```

---

## Full Example

```toml
# Team role assignment
role = "backend"

[provisioner]
git = "latest"
docker = "latest"
node = "20"
python = "3.12"
go = "1.22"
kubectl = "latest"
helm = "latest"

[hooks]
pre_setup = "echo 'Provisioning backend environment...'"
post_setup = "echo 'Ready to develop!'"

[hooks.node]
post_install = "npm install -g typescript"

[env.vars]
NODE_ENV = "development"
GO111MODULE = "on"

[services]
enabled = true
auto_start = true

[git]
default_branch = "main"
pull_rebase = true

[git.aliases]
co = "checkout"
st = "status"

[drift]
enabled = true
version_policy = "minor"

[roles.base]
tools = ["git", "docker"]

[roles.backend]
extends = "base"
tools = ["go", "python", "kubectl", "helm"]

[roles.frontend]
extends = "base"
tools = ["node", "bun"]
```
