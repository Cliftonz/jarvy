# Advanced Usage FAQ

## How do hooks work?

Hooks run shell commands after tool installation. They're useful for post-install configuration.

### Simple hook

```toml
[hooks]
node = "npm install -g yarn typescript"
rust = "rustup component add clippy rustfmt"
```

### Detailed hook with timeout

```toml
[hooks.docker]
script = '''
docker network create dev-network || true
docker volume create dev-data || true
'''
timeout = 120
```

### Default hooks

Many tools have built-in default hooks that configure shell initialization, completions, etc. These run automatically unless you override them.

```bash
# See tools with default hooks
jarvy tools --default-hooks
```

Default hooks are:
- **Idempotent**: Safe to run multiple times
- **Advisory**: Failures are warnings, not errors
- **Overridable**: Your custom hook takes precedence

## What are roles and how do I use them?

Roles define tool sets for different developer profiles, enabling team-wide standardization.

### Defining roles

```toml
[roles.base]
description = "Core development tools"
tools = ["git", "docker", "jq"]

[roles.frontend]
extends = "base"  # Inherits from base
description = "Frontend development"
tools = ["node", "bun", "prettier"]

[roles.frontend.tools]  # Version overrides
node = "20"
bun = "latest"

[roles.backend]
extends = "base"
tools = ["go", "postgresql", "redis"]
```

### Using roles

```toml
# Assign a single role
role = "frontend"

# Or multiple roles (tools merge, last wins for versions)
role = ["base", "frontend"]
```

### Role commands

```bash
jarvy roles list                    # List available roles
jarvy roles list -v                 # Verbose with tool counts
jarvy roles show frontend           # Show role details
jarvy roles show frontend --resolved   # Show with inherited tools
jarvy roles show frontend --inheritance  # Show inheritance chain
jarvy roles diff frontend backend   # Compare two roles
jarvy setup --role backend          # Override role for single run
```

### Inheritance

Roles can extend other roles up to 5 levels deep:

```
base → frontend → senior-frontend → lead-frontend
```

## How do I use Jarvy in CI/CD?

Jarvy auto-detects CI environments and adjusts behavior.

### GitHub Actions

```yaml
name: CI
on: [push, pull_request]

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Jarvy
        run: curl -fsSL https://jarvy.dev/install.sh | bash

      - name: Setup development tools
        run: jarvy setup --ci

      - name: Run tests
        run: cargo test
```

### GitLab CI

```yaml
setup:
  script:
    - curl -fsSL https://jarvy.dev/install.sh | bash
    - jarvy setup --ci
```

### CI-specific behavior

The `--ci` flag (or `CI=true` environment variable):
- Disables interactive prompts
- Enables non-interactive package manager flags
- Skips telemetry collection
- Uses appropriate output formatting

### Check CI detection

```bash
jarvy ci-info
```

Shows detected CI provider and environment details.

## How do tool dependencies work?

Tools can declare dependencies to ensure correct installation order.

### Strict dependencies

ALL listed tools must be available:

```rust
// lazydocker requires Docker
depends_on: &["docker"]
```

### Flexible dependencies

AT LEAST ONE tool must be available:

```rust
// kubectl works with any K8s cluster provider
depends_on_one_of: &["minikube", "kind", "k3d", "docker"]
```

### Dependency behavior

- **Strict**: Missing deps cause warnings; tool installs but may not work
- **Flexible**: Satisfied if any option is installed or in your config
- Dependencies affect installation order via topological sort

### Check dependencies

```bash
jarvy doctor          # Shows dependency satisfaction
jarvy validate        # Checks dependency configuration
```

### Ignore missing dependencies

For advanced users who manage dependencies externally:

```bash
jarvy setup --ignore-missing-deps
```

## What telemetry does Jarvy collect?

Telemetry is **opt-in** and disabled by default.

### What's collected (when enabled)

- Command usage statistics
- Tool installation success/failure rates
- Platform and Jarvy version
- Anonymous machine fingerprint (hashed, no PII)

### What's NOT collected

- Personal information
- File contents or paths
- Network traffic details
- Authentication tokens

### Managing telemetry

```bash
jarvy telemetry status     # Check current settings
jarvy telemetry enable     # Opt in
jarvy telemetry disable    # Opt out
jarvy telemetry preview    # See what would be sent
jarvy telemetry test       # Test OTLP endpoint connectivity
```

### Configuration

In `~/.jarvy/config.toml`:

```toml
[telemetry]
enabled = true
endpoint = "http://localhost:4318"
protocol = "http"  # or "grpc"
logs = true
metrics = true
traces = false
sample_rate = 1.0
```

Or via environment variables:

```bash
export JARVY_TELEMETRY=1
export JARVY_OTLP_ENDPOINT=http://localhost:4318
```

Telemetry is automatically disabled when `CI=true`.

## How do I use environment variables?

Jarvy can manage environment variables and secrets.

### Define variables

```toml
[env.vars]
NODE_ENV = "development"
DATABASE_URL = "postgres://localhost/myapp"

[env.secrets]
API_KEY = { from = "env", name = "MY_API_KEY" }
DB_PASSWORD = { from = "file", path = "~/.secrets/db_password" }
```

### Generate .env file

```bash
jarvy env export > .env
```

### Load in shell

```bash
eval $(jarvy env export --shell)
```
