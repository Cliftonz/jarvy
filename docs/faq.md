---
title: "FAQ - Jarvy"
description: "Frequently asked questions about Jarvy, the cross-platform dev environment CLI."
tags:
  - reference

---

# Frequently Asked Questions

## General

### What is Jarvy?

Jarvy is a fast, Rust-based CLI tool that standardizes and automates local development environment setup. It reads a `jarvy.toml` config file and installs all required tools using native package managers.

### Is Jarvy free and open source?

Yes. Jarvy is MIT-licensed and hosted at [github.com/bearbinary/jarvy](https://github.com/bearbinary/jarvy).

### What platforms does Jarvy support?

Jarvy runs on macOS, Linux, and Windows. It uses native package managers on each platform: Homebrew on macOS, apt/dnf/pacman on Linux, and winget/Chocolatey on Windows.

## Installation

### How do I install Jarvy?

Five options:

1. **Official installer** — `curl -fsSL https://raw.githubusercontent.com/bearbinary/jarvy/main/dist/scripts/install.sh | bash` (macOS/Linux) or `irm https://raw.githubusercontent.com/bearbinary/jarvy/main/dist/scripts/install.ps1 | iex` (Windows PowerShell).
2. **Cargo** — `cargo install jarvy`
3. **Homebrew** — `brew install jarvy`
4. **Binary** — download from [GitHub Releases](https://github.com/bearbinary/jarvy/releases)
5. **Repo bootstrap script** — for projects that already ship a `jarvy.toml`, copy [`scripts/bootstrap.sh`](https://raw.githubusercontent.com/bearbinary/jarvy/main/scripts/bootstrap.sh) into the repo so contributors run a single `./scripts/bootstrap.sh` to install Jarvy *and* provision. See the [Quickstart](quickstart.md#or-one-command-repo-bootstrap).

### How do I update Jarvy?

Run `jarvy update` to check for and install the latest version. Jarvy auto-detects how it was installed and uses the same method to update.

## Configuration

### What does a jarvy.toml look like?

A minimal config:

```toml
[provisioner]
git = "latest"
node = "20"
docker = "latest"
```

### Can I use version ranges?

Yes. Jarvy supports semver operators: `">=1.0"`, `"^20"`, `"~3.12"`, `"=1.2.3"`, or `"latest"`.

### How do I add custom post-install scripts?

Use the `[hooks]` section:

```toml
[hooks.node]
post_install = "npm install -g typescript"
```

### Can I set environment variables?

Yes. Use the `[env.vars]` section:

```toml
[env.vars]
NODE_ENV = "development"
EDITOR = "vim"
```

## Teams

### How do roles work?

Define roles in `jarvy.toml` and assign them to developers:

```toml
role = "frontend"

[roles.frontend]
description = "Frontend development"
tools = ["node", "bun", "docker"]
```

### Can I share configs across projects?

Yes. Use `extends` to inherit from a remote config:

```toml
extends = "https://raw.githubusercontent.com/org/configs/main/base.toml"
```

## CI/CD

### Does Jarvy work in CI?

Yes. Jarvy auto-detects CI environments and switches to non-interactive mode. Use the `setup-jarvy` GitHub Action or run `jarvy setup --ci` directly.

### Which CI providers are supported?

GitHub Actions, GitLab CI, CircleCI, Azure DevOps, Bitbucket Pipelines, Travis CI, Jenkins, Buildkite, TeamCity, and AppVeyor.

## AI Integration

### Can AI agents use Jarvy?

Yes. Run `jarvy mcp` to start the MCP (Model Context Protocol) server. AI agents can then discover, check, and install tools via JSON-RPC.

### Where is the LLM reference?

See [llms.txt](https://github.com/bearbinary/jarvy/blob/main/llms.txt) for a structured reference optimized for large language models.
