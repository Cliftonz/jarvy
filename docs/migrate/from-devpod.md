---
title: "Migrate: DevPod → Jarvy"
description: "Move from DevPod (devcontainer.json + Docker/K8s providers) to a native Jarvy setup. Translate features, postCreateCommand, and container env into jarvy.toml."
tags:
  - migrate
  - container
  - devpod

---

# Migrating from DevPod to Jarvy

DevPod runs your dev environment in a container — locally via Docker, or remotely via Kubernetes, AWS, Azure, GCP, or your own SSH host. It reads `devcontainer.json` and provisions a container per project.

If you're using DevPod purely for the **standardized toolchain** (and you're paying for the cloud infrastructure underneath), Jarvy gives you the same toolchain consistency on the developer's actual laptop — no container, no daemon, no remote SSH, no per-developer cloud bill.

If you need **container isolation per project** (a Postgres for project A on port 5432 *and* a Postgres for project B on port 5432, side by side), keep DevPod for that — Jarvy doesn't replace container isolation. But Jarvy can manage the host toolchain that DevPod itself runs on.

---

## Conceptual mapping

| DevPod / `devcontainer.json` | `jarvy.toml` equivalent |
|---|---|
| `image: "..."` | Not applicable — Jarvy doesn't use containers; tools install on the host |
| `features: { "ghcr.io/.../node": { version: "20" } }` | `node = "20"` under `[provisioner]` |
| `features: { "ghcr.io/.../python": { version: "3.12" } }` | `python = "3.12"` |
| `features: { "ghcr.io/.../docker-in-docker": {} }` | `docker = "latest"` (no DinD needed — host has Docker) |
| `postCreateCommand` | `[hooks] post_setup` |
| `postStartCommand` | Run via `[commands]` block + IDE hook |
| `containerEnv` / `remoteEnv` | `[env.vars]` |
| `forwardPorts` | Not applicable — native, no port forwarding |
| Docker provider | Not applicable — native |
| Kubernetes / cloud provider | Not applicable — laptop is the env |
| `customizations.vscode.extensions` | Manual (commit `.vscode/extensions.json`); Jarvy doesn't manage editor plugins |

---

## Step 1: read your devcontainer.json

A typical DevPod / dev container config:

```json title=".devcontainer/devcontainer.json (before)"
{
  "name": "my-app",
  "image": "mcr.microsoft.com/devcontainers/base:ubuntu",
  "features": {
    "ghcr.io/devcontainers/features/node:1": { "version": "20" },
    "ghcr.io/devcontainers/features/python:1": { "version": "3.12" },
    "ghcr.io/devcontainers/features/docker-in-docker:2": {}
  },
  "postCreateCommand": "npm ci && npm run prepare",
  "containerEnv": {
    "NODE_ENV": "development",
    "PYTHONUNBUFFERED": "1"
  }
}
```

---

## Step 2: write jarvy.toml

```toml title="jarvy.toml (after)"
[provisioner]
node   = "20"
python = "3.12"
docker = "latest"

[hooks]
post_setup = "npm ci && npm run prepare"

[env.vars]
NODE_ENV         = "development"
PYTHONUNBUFFERED = "1"
```

That's a 1:1 translation. Run `jarvy setup` and you have the same toolchain — on the laptop, native.

---

## Step 3: handle the container-only bits

A few `devcontainer.json` features don't have direct equivalents because they exist for the container, not for native dev:

**`forwardPorts`** — not needed; your service binds to localhost on your laptop, browser/postman/curl reach it directly.

**`customizations.vscode.extensions`** — Jarvy doesn't auto-install editor plugins. Two options:

1. Commit `.vscode/extensions.json` with `"recommendations": [...]` — VS Code prompts contributors on open.
2. Use a `post_setup` hook: `code --install-extension dbaeumer.vscode-eslint`.

**`mounts`** — irrelevant; your filesystem is your filesystem.

**`waitFor: "postCreateCommand"`** — `jarvy setup` is synchronous; the equivalent semantics happen automatically.

---

## Step 4: replace per-project isolation (if you needed it)

DevPod's killer feature is one Docker container per project. If two projects both need Postgres on `:5432`, DevPod isolates them.

Jarvy on its own doesn't isolate ports. Solutions:

| You need | Use |
|---|---|
| Different Postgres versions per project | `[services]` block with non-default ports — see [Configuration reference](../configuration.md) |
| Process-level sandbox per project | Keep DevPod for the few projects that need it; use Jarvy for everything else |
| Filesystem-level isolation | Combine `direnv` with project-specific PATH manipulation, or stay on DevPod |
| Reproducibility | `jarvy drift accept` baseline + `version_policy = "exact"` gives version-pinned, drift-detected (not container-isolated) |

---

## Step 5: shut off the container

After `jarvy setup && jarvy doctor` is green:

```bash
devpod stop my-app          # tear down the container
devpod delete my-app        # remove DevPod's tracking
```

Keep `.devcontainer/devcontainer.json` if you have contributors who use Codespaces or Dev Containers — they're not mutually exclusive. If everyone's on Jarvy now, delete it.

---

## What you gain

- **Native performance** — no Docker overhead, no remote SSH latency, no VM
- **Zero cloud cost** — no Kubernetes pod, no AWS instance, no per-developer bill
- **Offline** — works on a plane
- **Editor-agnostic** — you're not locked into VS Code Remote-Containers
- **Cross-platform** — same `jarvy.toml` works on macOS, Linux, Windows
- **Drift detection** — DevPod doesn't snapshot

## What you give up

- **Container isolation** — projects share the host toolchain. If you need project-isolated Postgres, Redis, etc., keep DevPod or layer in Docker Compose for the data services only.
- **Ephemeral environments** — DevPod tears down and rebuilds the container per project; Jarvy is durable per machine.
- **Resource ceiling** — DevPod with a Kubernetes provider can give a contractor a 32-core machine; Jarvy is bounded by the laptop.
- **Identical environments across team** — DevPod's container guarantee is stricter than Jarvy's drift-detected version pinning.

---

## A reasonable hybrid

Keep DevPod for projects that genuinely need container isolation (e.g., the security team's shared sandbox). Use Jarvy for everything else, plus to provision Docker/DevPod itself on each laptop:

```toml title="jarvy.toml"
[provisioner]
docker = "latest"
devpod = "latest"
```

Now Jarvy bootstraps DevPod, and DevPod isolates the few projects that need it.

---

## Migrate with AI

Paste this into Claude, ChatGPT, Cursor, or any LLM. Drop your `devcontainer.json` between the `<<<` and `>>>` markers. Save the output as `jarvy.toml`.

````text title="Prompt: DevPod → Jarvy"
You are a config translator. Convert the DevPod devcontainer.json below into a
valid jarvy.toml file.

# Schema (jarvy.toml)
- Required: [provisioner] — table of registered tool names mapped to versions.
- Versions accept: "latest", "20", "^3.10", "~3.12", "=20.11.0".
- Detailed form: tool = { version = "20", version_manager = true }
- Optional: [npm] [pip] [cargo] [hooks] [env.vars] [env.secrets]
  [git] [network] [drift] [services] [telemetry] [commands]
  role = "name" + [roles.<name>]

# Tool-name canonicalization
- nodejs → node, python3 → python, aws-cli → awscli, azure-cli → azure_cli
- postgresql / postgres → psql, visual-studio-code → vscode, golang → go

# What does NOT translate
- image / Dockerfile / build → tools install on host
- features.docker-in-docker / docker-outside-of-docker → docker = "latest"
  (the host has Docker; DinD is unnecessary)
- forwardPorts, mounts, runArgs → not applicable on a native host
- customizations.vscode → user must commit .vscode/extensions.json
- DevPod providers (Docker / K8s / cloud) → host IS the provider
- container isolation per project → Jarvy doesn't isolate; flag this in a comment

# Per-source rules
- features.<feature>.version → tool = "version" under [provisioner]
- postCreateCommand / onCreateCommand → [hooks] post_setup
- containerEnv / remoteEnv → [env.vars]
- If config has multiple postCreate scripts, concatenate with ' && '

# Output contract
- Output ONLY the jarvy.toml content. No prose, no markdown fence.
- All hooks must be idempotent.

# INPUT
<<<
[paste your devcontainer.json here]
>>>
````

After the model responds, run `jarvy validate` and `jarvy setup --dry-run`.

---

## See also

- [vs DevPod](../competitors/vs-devpod.md) — feature comparison
- [Configuration reference](../configuration.md)
- [Drift detection](../drift.md)
