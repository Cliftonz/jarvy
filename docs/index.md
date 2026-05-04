---
title: "Jarvy - Dev Environment CLI"
description: "Jarvy is a fast, cross-platform CLI that standardizes and automates local development environment setup from a declarative jarvy.toml config file."
---

# What is Jarvy?

**Jarvy** is a fast, Rust-based CLI tool that standardizes and automates local development environment setup across macOS, Linux, and Windows.

It reads a declarative `jarvy.toml` config file in your repository and provisions all required tools using native package managers — Homebrew on macOS, apt/dnf on Linux, winget/Chocolatey on Windows.

**Stop paying for dev pods and codespaces.** Jarvy runs on your local machine with zero cloud costs, full offline support, and native performance.

## Install

=== "Cargo"

    ```bash
    cargo install jarvy
    ```

=== "Homebrew"

    ```bash
    brew install jarvy
    ```

=== "Binary"

    Download a pre-built binary from the
    [GitHub Releases](https://github.com/bearbinary/jarvy/releases) page.

## Quick Example

Create a `jarvy.toml` in your repository:

```toml
[provisioner]
git = "latest"
node = "20"
docker = "latest"
python = "3.12"
```

Then run:

```bash
jarvy setup
```

Every developer on the team gets the same tools, same versions, same environment.

## Why Jarvy?

| Problem | Jarvy's Solution |
|---------|-----------------|
| "Works on my machine" | Declarative config ensures identical environments |
| Days-long onboarding | New developers run `jarvy setup` and are done in seconds |
| Cloud dev environment costs | Provisions locally — no VM, no container, no recurring cost |
| Cross-platform drift | One config file works on macOS, Linux, and Windows |
| Manual setup guides | Environment as code, version-controlled in your repo |

## Key Features

- **174+ tools** supported out of the box
- **Role-based configurations** for team-specific tool sets
- **MCP server** for AI agent integration (Claude, GPT, etc.)
- **Drift detection** to catch environment changes
- **Post-install hooks** for automated configuration
- **CI/CD integration** with 11 providers auto-detected

## FAQ

**How many tools does Jarvy support?**

Jarvy supports 174+ tools including Node.js, Python, Go, Rust, Docker, Kubernetes, Terraform, AWS CLI, and many more. Run `jarvy search --all` to see the full list.

**Can AI agents use Jarvy?**

Yes. Jarvy includes a built-in MCP (Model Context Protocol) server. Run `jarvy mcp` to let AI agents discover and install tools via JSON-RPC.

**Is Jarvy free?**

Yes. Jarvy is open-source and MIT-licensed.
