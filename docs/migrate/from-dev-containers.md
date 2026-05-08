---
title: "Migrate: VS Code Dev Containers → Jarvy"
description: "Move from VS Code Dev Containers (devcontainer.json) to a native Jarvy setup. Translate features, lifecycle scripts, and editor extensions."
tags:
  - migrate
  - container
  - devcontainer

---

# Migrating from VS Code Dev Containers to Jarvy

VS Code Dev Containers run your project inside a Docker container so the editor connects to a pre-baked toolchain. The container guarantees everyone has the same tools, but the cost is Docker overhead, slow first builds, and editor lock-in to VS Code.

If you're using Dev Containers because **standardizing the toolchain across the team is hard**, Jarvy gives you the same toolchain consistency without the container — no Docker daemon to keep happy, no build-image step, no `Reopen in Container` round trip.

---

## Conceptual mapping

This is the same `devcontainer.json` schema used by Codespaces and DevPod, just with a local Docker provider:

| `devcontainer.json` | `jarvy.toml` equivalent |
|---|---|
| `image` / `Dockerfile` | Not applicable — tools install on the host |
| `features` | `[provisioner]` |
| `onCreateCommand` / `postCreateCommand` | `[hooks] post_setup` |
| `postStartCommand` / `postAttachCommand` | `[commands]` block |
| `containerEnv` / `remoteEnv` | `[env.vars]` |
| `mounts` | Not needed — your filesystem is your filesystem |
| `forwardPorts` | Not applicable — services bind to localhost |
| `customizations.vscode.extensions` | Commit `.vscode/extensions.json` with `recommendations` |
| `runArgs` (Docker flags) | Not applicable |
| `workspaceFolder` | Not applicable — repo lives at its real path |

---

## Step 1: read your devcontainer.json

```json title=".devcontainer/devcontainer.json (before)"
{
  "name": "myapp",
  "build": { "dockerfile": "Dockerfile" },
  "features": {
    "ghcr.io/devcontainers/features/go:1": { "version": "1.22" },
    "ghcr.io/devcontainers/features/node:1": { "version": "20" },
    "ghcr.io/devcontainers/features/docker-outside-of-docker:1": {}
  },
  "postCreateCommand": "go mod download && npm ci",
  "postAttachCommand": "echo 'ready'",
  "remoteEnv": {
    "GOFLAGS": "-mod=readonly"
  },
  "customizations": {
    "vscode": {
      "extensions": [
        "golang.go",
        "dbaeumer.vscode-eslint"
      ],
      "settings": {
        "go.useLanguageServer": true
      }
    }
  }
}
```

---

## Step 2: write jarvy.toml

```toml title="jarvy.toml"
[provisioner]
go     = "1.22"
node   = "20"
docker = "latest"

[hooks]
post_setup = "go mod download && npm ci"

[env.vars]
GOFLAGS = "-mod=readonly"

[commands]
ready = "echo ready"
```

Move VS Code-specific settings to the editor's normal config files:

```json title=".vscode/extensions.json"
{
  "recommendations": ["golang.go", "dbaeumer.vscode-eslint"]
}
```

```json title=".vscode/settings.json"
{
  "go.useLanguageServer": true
}
```

If you want the settings file tracked for drift, add it to `[drift]`:

```toml
[drift]
enabled     = true
track_files = [".vscode/settings.json", ".vscode/extensions.json"]
```

---

## Step 3: handle the Dockerfile

If you had a custom `Dockerfile` (not just features), translate each `RUN` line:

| Dockerfile pattern | Translation |
|---|---|
| `apt-get install <tool>` | `<tool>` in `[provisioner]` if Jarvy knows it; else `pre_setup` hook |
| `pip install <pkg>` | `[pip]` block |
| `npm install -g <pkg>` | `[npm]` block |
| `curl ... \| bash` (tool installer) | `pre_setup` hook, or contribute the tool — see [Adding tools](../adding-tools.md) |
| `COPY ./scripts /usr/local/bin/` | Keep the script in `scripts/`, reference it from `[hooks]` |

---

## Step 4: replace lifecycle scripts

Dev Containers have four lifecycle hooks. Map them:

| Dev Container hook | Jarvy hook |
|---|---|
| `initializeCommand` | `pre_setup` (runs once before installs) |
| `onCreateCommand` | `post_setup` |
| `postCreateCommand` | `post_setup` (combine if both exist) |
| `postStartCommand` | `[commands]` entry, run on demand |
| `postAttachCommand` | `[commands]` entry, run on demand |

The lifecycle's "create vs start vs attach" distinction collapses on a native machine — there's only "set up" and "run."

---

## Step 5: ship the README change

```markdown title="README.md"
## Local development

This repo supports both local-native and Dev Container workflows.

**Native (recommended):**

```bash
curl -fsSL https://raw.githubusercontent.com/bearbinary/jarvy/main/dist/scripts/install.sh | bash
jarvy setup
```

**Dev Container:** open in VS Code and "Reopen in Container."
```

If everyone is on Jarvy, delete `.devcontainer/`. If some contributors stay on Dev Containers, keep both files in sync (the schemas are similar enough that a script can do this).

---

## What you gain

- **No Docker daemon** — required for Dev Containers, optional for Jarvy (only if your project actually uses Docker)
- **Native filesystem** — no bind-mount weirdness, no permission shifts on macOS/Windows
- **Faster first run** — no image build, no layer pulls
- **Editor agnostic** — works with VS Code, JetBrains, Vim, Emacs, anything
- **Drift detection** — Dev Containers don't snapshot
- **Smaller cognitive load** — one mental model, not "is my issue in the container or the host?"

## What you give up

- **Container isolation** — projects share the host toolchain. Two projects needing different system libraries may fight.
- **Identical-environment guarantee** — container > native for strict reproducibility
- **The "Reopen in Container" muscle memory** — you'll be running `jarvy setup` instead

---

## Migrate with AI

Paste this into Claude, ChatGPT, Cursor, or any LLM. Drop your `devcontainer.json` between the `<<<` and `>>>` markers.

````text title="Prompt: Dev Containers → Jarvy"
You are a config translator. Convert the VS Code Dev Container devcontainer.json
below into a valid jarvy.toml file.

# Schema (jarvy.toml)
- Required: [provisioner] — table of registered tool names mapped to versions.
- Versions: "latest", "20", "^3.10", "~3.12", "=20.11.0".
- Detailed: tool = { version = "20", version_manager = true }
- Optional: [npm] [pip] [cargo] [hooks] [env.vars] [env.secrets]
  [git] [network] [drift] [services] [telemetry] [commands]

# Tool-name canonicalization
- nodejs → node, python3 → python, aws-cli → awscli, azure-cli → azure_cli
- postgresql / postgres → psql, visual-studio-code → vscode, golang → go

# What does NOT translate
- image / build / Dockerfile → tools install on host
- features.docker-in-docker / docker-outside-of-docker → docker = "latest"
- mounts, runArgs, workspaceFolder → not applicable on a native host
- forwardPorts → services bind to localhost natively
- customizations.vscode.extensions → user commits .vscode/extensions.json
- customizations.vscode.settings → user commits .vscode/settings.json,
  optionally add to [drift] track_files for drift detection

# Per-source rules
- features.<feature>.version → tool = "version" under [provisioner]
- onCreateCommand + postCreateCommand → [hooks] post_setup (combine if both)
- postStartCommand / postAttachCommand → [commands] entry
- containerEnv / remoteEnv → [env.vars]
- If .vscode/settings.json or .vscode/extensions.json is referenced, add a
  [drift] block with track_files including those paths.

# Output contract
- Output ONLY the jarvy.toml content. No prose, no fence.
- All hooks idempotent.

# INPUT
<<<
[paste your devcontainer.json here]
>>>
````

After the model responds, run `jarvy validate` and `jarvy setup --dry-run`.

---

## See also

- [Migrate from DevPod](from-devpod.md) — same `devcontainer.json` source, K8s/cloud runtime
- [Migrate from Codespaces](from-codespaces.md) — same source, GitHub-hosted runtime
- [vs Docker](../competitors/vs-docker.md) — when containers are the right answer
