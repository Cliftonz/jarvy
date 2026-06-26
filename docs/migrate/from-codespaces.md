---
title: "Migrate: GitHub Codespaces → Jarvy"
description: "Move from Codespaces to local Jarvy. Translate devcontainer.json features, postCreateCommand, lifecycle scripts, and secrets — and stop paying per-hour."
tags:
  - migrate
  - cloud
  - codespaces

---

# Migrating from GitHub Codespaces to Jarvy

Codespaces gives every developer a cloud-hosted VM with a pre-configured environment. The selling points are zero local setup, fresh per-PR environments, and the ability to scale up to 32-core machines on demand. The downsides are the per-hour bill, the requirement of a stable internet connection, and the friction of working through a remote IDE.

If you're using Codespaces mainly because **onboarding is slow** and **environments drift**, Jarvy fixes both — locally, with no cloud cost. If you're using Codespaces for **ephemeral PR environments** or **scale-up hardware**, you'll keep some of it; Jarvy can complement, not replace.

---

## Conceptual mapping

Codespaces reads `.devcontainer/devcontainer.json` plus optional Codespaces-specific settings. Translation:

| Codespaces config | `jarvy.toml` equivalent |
|---|---|
| `image` / `Dockerfile` | Not applicable — Jarvy installs on the host |
| `features` | `[provisioner]` |
| `postCreateCommand` | `[hooks] post_setup` |
| `postStartCommand` / `postAttachCommand` | `[commands]` block + a `direnv` script if you want auto-run on `cd` |
| `containerEnv` | `[env.vars]` |
| Codespaces secrets | `[env.secrets]` with `{ env = "VAR", required = true }` |
| `prebuildConfiguration` | Not applicable — `jarvy setup` is fast enough that prebuilds aren't needed |
| `forwardPorts`, `portsAttributes` | Not applicable — native; ports are local |
| `customizations.vscode.extensions` | Commit `.vscode/extensions.json` with `recommendations` |
| Machine type (cores/RAM) | Whatever the laptop has |

---

## Step 1: read your devcontainer.json

```json title=".devcontainer/devcontainer.json"
{
  "name": "myapp",
  "image": "mcr.microsoft.com/devcontainers/javascript-node:20",
  "features": {
    "ghcr.io/devcontainers/features/python:1": { "version": "3.12" },
    "ghcr.io/devcontainers/features/aws-cli:1": {}
  },
  "postCreateCommand": "npm ci",
  "postAttachCommand": "npm run dev",
  "containerEnv": {
    "NODE_ENV": "development"
  },
  "customizations": {
    "vscode": {
      "extensions": ["dbaeumer.vscode-eslint", "esbenp.prettier-vscode"]
    }
  }
}
```

---

## Step 2: write jarvy.toml

```toml title="jarvy.toml"
[provisioner]
node    = "20"
python  = "3.12"
awscli = "latest"

[npm]
typescript = "^5.0"
prettier   = "latest"

[hooks]
post_setup = "npm ci"

[env.vars]
NODE_ENV = "development"

# Codespaces secrets → declare them here, source them from your shell or a vault
[env.secrets]
GITHUB_TOKEN = { env = "GITHUB_TOKEN", required = true }
```

For the VS Code extensions, commit them as recommendations:

```json title=".vscode/extensions.json"
{
  "recommendations": [
    "dbaeumer.vscode-eslint",
    "esbenp.prettier-vscode"
  ]
}
```

---

## Step 3: handle Codespaces-only features

**`postAttachCommand`** runs every time you reconnect to a Codespace. The closest local equivalent is a `[commands]` block plus a shell `cd` hook (e.g., direnv):

```toml title="jarvy.toml"
[commands]
dev   = "npm run dev"
test  = "npm test"
build = "npm run build"
```

Run `jarvy` (no subcommand) and pick `dev` from the menu — replaces the auto-run-on-attach behavior.

**Codespaces secrets** are user-scoped or org-scoped vault entries. After migrating to Jarvy, decide where secrets live:

- **Per-developer:** developers source them from `~/.zshrc`, `~/.bashrc`, or a tool like `direnv` / `op` / `gpg`.
- **Per-project:** use `[env.secrets]` with `{ env = "VAR", required = true }` so Jarvy fails loudly if a contributor hasn't set up the secret.

```toml
[env.secrets]
DATABASE_URL  = { env = "DATABASE_URL", required = true }
OPENAI_API_KEY = { env = "OPENAI_API_KEY", required = false }
```

**Prebuilds** don't have a local equivalent. They're not needed — `jarvy setup` is seconds-fast on cached package managers, and tools-already-installed are skipped. The motivation for prebuilds (multi-minute container builds) doesn't apply.

---

## Step 4: keep ephemeral environments where they earn their keep

Some teams use Codespaces specifically for **per-PR ephemeral environments** — every pull request spins up a clean Codespace for review. That use case is worth keeping if it's working.

Hybrid:

- Local development → Jarvy (every laptop, fast, free)
- Ephemeral PR review → Codespaces (small instances, short-lived)
- Both read the same `devcontainer.json` *and* `jarvy.toml`. Most fields can be kept in sync by hand or by a generator script.

You stop paying for daily-driver Codespaces; you keep paying only for the per-PR previews where the cloud model actually wins.

---

## Step 5: ship the README change

Update your project README:

```markdown title="README.md"
## Local development

Prefer to develop locally? You don't need Codespaces.

```bash
curl -fsSL https://raw.githubusercontent.com/Cliftonz/jarvy/main/dist/scripts/install.sh | bash
jarvy setup
```

That's it. Same toolchain as Codespaces, on your laptop, no per-hour bill.
```

---

## What you gain

- **No per-hour bill** — Codespaces at the [advertised $0.18-$2.88/hour](https://docs.github.com/en/billing/managing-billing-for-github-codespaces/about-billing-for-github-codespaces) adds up to real money over a year per developer
- **Native performance** — no remote IDE latency, no terminal lag, no "running on a 2-core machine somewhere"
- **Offline** — code on a plane, on a train, in a coffee shop with bad wifi
- **Editor agnostic** — VS Code, JetBrains, Vim, Emacs, anything
- **Privacy** — code lives on the laptop, not in someone else's cloud
- **No "Codespace stopped" surprises** — your local env doesn't time out

## What you give up

- **Per-PR ephemeral environments** — keep Codespaces for that use case if you have it
- **Scale-up hardware** — a contractor's old laptop is a contractor's old laptop
- **Truly identical environments** — Codespaces' container guarantee is stricter than Jarvy's drift-detected version pinning
- **Zero local setup** — Jarvy still requires running an install command once per laptop

---

## Migrate with AI

Paste this into Claude, ChatGPT, Cursor, or any LLM. Drop your `devcontainer.json` between the `<<<` and `>>>` markers. Save the output as `jarvy.toml`.

````text title="Prompt: Codespaces → Jarvy"
You are a config translator. Convert the GitHub Codespaces devcontainer.json
below into a valid jarvy.toml file.

# Schema (jarvy.toml)
- Required: [provisioner] — table of registered tool names mapped to versions.
- Versions accept: "latest", bare major like "20", caret "^3.10", tilde "~3.12",
  or exact "=20.11.0".
- Detailed form: tool = { version = "20", version_manager = true }
- Optional sections:
    [npm], [pip], [cargo]                  # language packages
    [hooks] pre_setup, post_setup          # global, run once
    [hooks.<tool>] post_install            # per-tool
    [env.vars] KEY = "value"               # written to .env + shell rc
    [env.secrets] KEY = { env = "VAR", required = true }
    [git], [network], [drift], [services], [telemetry], [commands]
    role = "name" + [roles.<name>]         # role-based bundles

# Tool-name canonicalization (use exact names)
- nodejs → node
- python3 → python
- aws-cli → awscli
- azure-cli → azure_cli
- postgresql / postgres → psql
- visual-studio-code → vscode
- golang → go

# What does NOT translate (omit it; don't fake it)
- image / build.dockerfile  → tools install on the host, no container
- forwardPorts / portsAttributes  → services bind to localhost natively
- mounts / workspaceFolder  → filesystem is the filesystem
- customizations.vscode.extensions  → user must commit .vscode/extensions.json
- prebuildConfiguration  → not needed; jarvy setup is fast enough
- machine type / hardware specs  → laptop has what it has

# Per-source rules
- features.<feature> → tool entry under [provisioner]
- ghcr.io/devcontainers/features/docker-in-docker → docker = "latest"
- postCreateCommand / onCreateCommand → [hooks] post_setup
- postAttachCommand / postStartCommand → [commands] entry (e.g. dev = "...")
- containerEnv / remoteEnv → [env.vars]
- Codespaces user/org secrets → [env.secrets] with required = true

# Output contract
- Output ONLY the jarvy.toml content. No prose, no markdown fence, no commentary.
- All hooks must be idempotent.
- If unsure whether a tool is registered, prefer the closest canonical name.

# INPUT
<<<
[paste your devcontainer.json here]
>>>
````

After the model responds, run `jarvy validate` and `jarvy setup --dry-run` to confirm the output is correct.

---

## See also

- [vs Codespaces](../competitors/vs-codespaces.md) — feature comparison
- [Migrate from DevPod](from-devpod.md) — same `devcontainer.json` source, different runtime
- [Tutorial — onboard a team](../tutorials/team-onboarding.md)
