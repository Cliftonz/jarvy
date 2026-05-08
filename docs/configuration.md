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
