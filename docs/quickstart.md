---
title: "Quickstart - Jarvy"
description: "Get started with Jarvy in under 5 minutes. Install, configure, and provision your development environment."
---

# Quickstart

Get your development environment standardized in under 5 minutes.

## 1. Install Jarvy

=== "Cargo"

    ```bash
    cargo install jarvy
    ```

=== "Homebrew"

    ```bash
    brew install jarvy
    ```

=== "Binary"

    Download from [GitHub Releases](https://github.com/bearbinary/jarvy/releases) and add to your PATH.

## 2. Create a Config

=== "Interactive"

    ```bash
    jarvy init
    ```

    Follow the prompts to select your tools.

=== "From Template"

    ```bash
    jarvy init --template react
    ```

    Available templates: `react`, `vue`, `go-api`, `rust-cli`, `python-ml`, `devops`, and more.

=== "Manual"

    Create `jarvy.toml` in your project root:

    ```toml
    [provisioner]
    git = "latest"
    node = "20"
    docker = "latest"
    python = "3.12"

    [env.vars]
    NODE_ENV = "development"

    [hooks.node]
    post_install = "npm install -g typescript eslint"
    ```

## 3. Run Setup

```bash
jarvy setup
```

Jarvy installs all tools, runs hooks, and configures your environment.

## 4. Verify

```bash
jarvy doctor
```

Check that all tools are installed and at the correct versions.

## Configuration Reference

### Tool Versions

```toml
[provisioner]
# Simple version
node = "20"

# Latest available
docker = "latest"

# Detailed config
python = { version = "3.12", version_manager = true }
```

### Environment Variables

```toml
[env.vars]
NODE_ENV = "development"
API_URL = "http://localhost:3000"

[env.secrets]
API_KEY = { env = "MY_API_KEY", required = true }
```

### Hooks

```toml
[hooks]
pre_setup = "echo 'Starting setup...'"
post_setup = "echo 'Setup complete!'"

[hooks.node]
post_install = "npm install -g typescript"
```

### Roles

```toml
role = "frontend"

[roles.base]
tools = ["git", "docker"]

[roles.frontend]
extends = "base"
tools = ["node", "bun"]
```

### Services

```toml
[services]
enabled = true
auto_start = true
```

### Drift Detection

```toml
[drift]
enabled = true
version_policy = "minor"
```

## Next Steps

- Run `jarvy search --all` to browse 174+ available tools
- Run `jarvy explain <tool>` for detailed tool information
- Run `jarvy templates list` to browse configuration templates
- Read the [FAQ](faq.md) for common questions
