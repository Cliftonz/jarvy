---
title: "Quickstart — Jarvy"
description: "Install Jarvy and provision your first environment in under 60 seconds. The TL;DR for impatient developers."
tags:
  - getting-started

---

# Quickstart

The TL;DR. Want a guided walkthrough instead? See the [tutorial: your first jarvy.toml](tutorials/first-config.md).

---

## Install

=== "macOS / Linux"

    ```bash
    curl -fsSL https://raw.githubusercontent.com/bearbinary/jarvy/main/dist/scripts/install.sh | bash
    ```

=== "Homebrew"

    ```bash
    brew install jarvy
    ```

=== "Cargo"

    ```bash
    cargo install jarvy
    ```

=== "Windows (PowerShell)"

    ```powershell
    irm https://raw.githubusercontent.com/bearbinary/jarvy/main/dist/scripts/install.ps1 | iex
    ```

=== "Binary"

    Download from [GitHub Releases](https://github.com/bearbinary/jarvy/releases) and add to `PATH`.

Verify: `jarvy --version`

---

## Configure

Drop a `jarvy.toml` in your repo root:

```toml title="jarvy.toml"
[provisioner]
git    = "latest"
node   = "20"
python = "3.12"
docker = "latest"

[hooks.node]
post_install = "npm install -g typescript"

[env.vars]
NODE_ENV = "development"
```

Or start from a [template](templates-index.md): `jarvy init --template node-pnpm`

---

## Provision

```bash
jarvy setup
```

That's it. Jarvy installs missing tools, runs hooks, writes `.env`, and snapshots the result for [drift detection](drift.md).

---

## Verify

```bash
jarvy doctor
```

Walks every tool, confirms version satisfies the config.

---

## Useful commands

| Command | What it does |
|---|---|
| `jarvy diff` | What would change on this machine |
| `jarvy setup --dry-run` | Full plan, no execution |
| `jarvy validate` | Schema check on `jarvy.toml` |
| `jarvy doctor` | Verify everything is installed correctly |
| `jarvy drift check` | Compare current machine to the committed baseline |
| `jarvy drift accept` | Update the baseline to current state |
| `jarvy tools` | List all 200+ supported tools |
| `jarvy search <name>` | Find a tool in the registry |
| `jarvy explain <name>` | Detailed metadata for one tool |
| `jarvy templates list` | Browse starter `jarvy.toml` files |
| `jarvy update` | Self-update Jarvy |
| `jarvy mcp` | Start the MCP server for AI agents |

[Full CLI reference →](cli.md)

---

## Next

- **5-minute tutorial:** [Your first jarvy.toml](tutorials/first-config.md)
- **Onboarding a team:** [Tutorial — onboard a team in 10 minutes](tutorials/team-onboarding.md)
- **Mental model:** [Concepts overview](concepts/overview.md)
- **All options:** [Configuration reference](configuration.md)
- **Migrating from another tool:** [Migration guides](migrate/index.md)
