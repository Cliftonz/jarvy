---
title: "Tutorial: Onboard a team in 10 minutes — Jarvy"
description: "Take an existing repo and ship a jarvy.toml that gets every contributor — new hires, contractors, OSS contributors — to a working dev environment with one command."
tags:
  - tutorial
  - team

---

# Tutorial: onboard a team in 10 minutes

**Time:** ~10 minutes · **You'll need:** an existing repository you can commit to, and one teammate to test the result with.

By the end you'll have:

- A `jarvy.toml` that captures your project's full toolchain
- A `README.md` snippet that turns onboarding into a single command
- A baseline snapshot so future drift is visible
- A CI check so config drift never reaches `main`

This tutorial assumes you've already done the [first-config tutorial](first-config.md) on your own machine.

---

## 1. Audit what's actually on your machine

Before you write the config, capture what your project already needs:

```bash
cd path/to/your/repo
jarvy init --interactive
```

Jarvy scans for common signals — `package.json`, `requirements.txt`, `Cargo.toml`, `go.mod`, `Dockerfile`, `terraform/` — and proposes a starter config. Edit it and save.

If you'd rather start from a blank file:

```bash
jarvy init --template node-pnpm   # or react, python-uv, go-api, rust-cli, …
```

[Browse the 14 starter templates →](../templates-index.md)

---

## 2. Promote tribal knowledge into the config

Walk through your project's existing onboarding docs and translate each step:

| Wiki / README step | `jarvy.toml` translation |
|---|---|
| "Install Node 20" | `node = "20"` under `[provisioner]` |
| "Install pnpm globally" | `[npm]` block with `pnpm = "latest"`, or `package_manager = "pnpm"` |
| "Run `pnpm install`" | `[hooks.node] post_install = "pnpm install"` |
| "Set `DATABASE_URL`" | `[env.vars]` or `[env.secrets]` with `{ env = "DATABASE_URL", required = true }` |
| "Run `make db-seed`" | `[hooks] post_setup = "make db-seed"` |
| "Configure git signing" | `[git]` block — see [Git configuration](../git-config.md) |
| "Behind corporate proxy?" | `[network]` block — see [Network & proxy](../network.md) |

The point: if it's in a wiki page that drifts, it belongs in `jarvy.toml`.

---

## 3. Split by role if your team has more than one job

A monorepo with frontend, backend, and DevOps contributors doesn't need every laptop to install kubectl. Use [roles](../roles.md):

```toml title="jarvy.toml"
role = "frontend"   # default for this checkout

[roles.base]
description = "Tools every contributor needs"
tools = ["git", "docker"]

[roles.frontend]
extends = "base"
tools = ["node", "pnpm"]

[roles.backend]
extends = "base"
tools = ["python", "postgres"]

[roles.devops]
extends = "base"
tools = ["kubectl", "helm", "terraform"]
```

Each contributor overrides for a single run with `jarvy setup --role devops` or commits a per-checkout default.

---

## 4. Validate locally

```bash
jarvy validate           # schema check
jarvy diff               # compare config to current machine
jarvy setup --dry-run    # full plan, no execution
```

Fix any errors before committing — the `--strict` flag will fail loudly on unknowns.

---

## 5. Commit and ship the README snippet

Add this to your repo's `README.md`:

````markdown
## Set up the dev environment

```bash
# Install Jarvy (one-time, per laptop)
curl -fsSL https://raw.githubusercontent.com/bearbinary/jarvy/main/dist/scripts/install.sh | bash

# Provision this project's tools
jarvy setup
```

That's it. Re-run `jarvy setup` any time you need to refresh the environment.
````

### Even simpler: ship `scripts/bootstrap.sh`

Want a true one-command onboarding? Drop Jarvy's bootstrap script into your repo:

```bash
mkdir -p scripts
curl -fsSL \
  https://raw.githubusercontent.com/bearbinary/jarvy/main/scripts/bootstrap.sh \
  -o scripts/bootstrap.sh
chmod +x scripts/bootstrap.sh
```

Then your README snippet collapses to:

````markdown
## Set up the dev environment

```bash
./scripts/bootstrap.sh
```

Installs Jarvy if missing, then runs `jarvy setup`. Idempotent — safe to re-run.
````

The script handles the install-Jarvy-first hop, adds `~/.cargo/bin`, `~/.local/bin`, `/usr/local/bin`, and `/opt/homebrew/bin` to `PATH` if needed, and runs `jarvy setup` against the `jarvy.toml` at the repo root. Flags: `--no-setup`, `--channel beta|nightly`, plus passthrough args to `jarvy setup` (e.g. `./scripts/bootstrap.sh --role devops`).

Commit `jarvy.toml`, `scripts/bootstrap.sh`, and the README change.

---

## 6. Capture the baseline

Right after a clean `jarvy setup`, run:

```bash
jarvy drift accept
```

This writes `.jarvy/state.json` — a snapshot of the exact tool versions and tracked file hashes on a known-good machine. Commit it.

From here on, anyone can run `jarvy drift check` and see whether their machine has drifted from the baseline.

---

## 7. Add a CI check

Add a workflow that catches drift before it merges. Example for GitHub Actions:

```yaml title=".github/workflows/jarvy.yml"
name: Jarvy
on: [pull_request]

jobs:
  validate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: curl -fsSL https://raw.githubusercontent.com/bearbinary/jarvy/main/dist/scripts/install.sh | bash
      - run: jarvy validate --strict
      - run: jarvy diff --format json    # fails if config drifts from lockfile
```

For provider-specific snippets (GitLab, CircleCI, Buildkite, Jenkins), see the [CI/CD guide](../ci-cd.md).

---

## 8. Test with a teammate

Have one teammate clone the repo on a clean (or clean-ish) laptop:

```bash
git clone <repo>
cd <repo>
jarvy setup
jarvy doctor
```

`doctor` should print all green. If it doesn't, that gap is exactly the kind of tribal knowledge `jarvy.toml` is supposed to absorb — capture it now while it's fresh.

---

## What just happened

Your repo went from "follow these 12 wiki steps" to one command. The config is reviewed in PRs, drifts are caught in CI, and new contributors are productive in minutes.

---

## Next steps

- **Pin versions tighter for releases:** swap `node = "20"` for `node = "20.11.1"`
- **Move secrets out of the file:** [`env.secrets` with `{ env = "VAR" }` indirection](../configuration.md#environment-variables-env)
- **Track config drift with telemetry:** [Telemetry guide](../telemetry.md)
- **Generate a debug bundle when something breaks:** `jarvy ticket create` — [Logging & tickets](../logging.md)
- **Compare to existing tools you may be replacing:** [vs Codespaces](../competitors/vs-codespaces.md), [vs mise](../competitors/vs-mise.md), [vs Homebrew Bundle](../competitors/vs-homebrew-bundle.md)
