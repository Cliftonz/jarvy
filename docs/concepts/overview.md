---
title: "Concepts overview — Jarvy"
description: "The mental model behind Jarvy: how the config, tools, hooks, roles, and drift state fit together."
tags:
  - concept

---

# Concepts overview

Jarvy is a small system. There are five ideas that explain everything else; once you have these five, the [reference](../configuration.md) reads itself.

```mermaid
flowchart LR
    subgraph "What you write"
      A[jarvy.toml] -.optional.- R[roles]
    end
    A --> B(jarvy CLI)
    R --> B
    subgraph "What Jarvy runs"
      B --> C[Tool resolver]
      C --> D[Native package manager]
      D --> E[Hooks]
      E --> F[Env + Git config]
      F --> G[(.jarvy/state.json baseline)]
    end
    G -.- H[drift check]
```

| Concept | One-liner | Deep dive |
|---|---|---|
| **Config** | `jarvy.toml` is the source of truth — every option is declarative and git-versioned. | [Reference](../configuration.md) |
| **Tools** | Every tool is a recipe Jarvy knows how to install with the local package manager. | [Tools](tools.md) |
| **Lifecycle** | `jarvy setup` runs a fixed sequence — resolve, install, hook, env, snapshot. | [Lifecycle](lifecycle.md) |
| **Roles** | A role is a named bundle of tools that can extend other roles, with per-role version overrides. | [Roles](roles-and-inheritance.md) |
| **Drift baseline** | After a clean setup, Jarvy records exact versions and file hashes; later it compares. | [Drift baseline](drift-baseline.md) |

---

## How a `jarvy setup` actually flows

```mermaid
sequenceDiagram
    participant User
    participant CLI as jarvy CLI
    participant Reg as Tool registry
    participant PM as Package manager
    participant Disk

    User->>CLI: jarvy setup
    CLI->>Disk: read jarvy.toml
    CLI->>CLI: resolve role inheritance
    CLI->>Reg: look up each tool
    Reg-->>CLI: install recipes
    CLI->>CLI: topo-sort by depends_on
    loop for each tool
        CLI->>PM: install (or skip if present)
        PM-->>CLI: ok
        CLI->>CLI: run post_install hook
    end
    CLI->>Disk: write .env + git config
    CLI->>Disk: write .jarvy/state.json
    CLI-->>User: ✓ done
```

Every step is idempotent — running setup twice is always safe.

---

## Where things live on disk

| Path | Purpose | Versioned? |
|---|---|---|
| `jarvy.toml` | Project config — tools, hooks, roles, env. | Yes — commit to repo |
| `.jarvy/state.json` | Drift baseline — exact versions captured after `jarvy setup`. | Yes — commit to repo |
| `~/.jarvy/config.toml` | Per-machine settings: telemetry, update channel. | No — user preference |
| `~/.jarvy/logs/` | Rotated log files. | No |
| `~/.jarvy/tickets/` | Diagnostic bundles for support. | No |

---

## The five sections you'll see in every config

```toml
[provisioner]   # tools to install — see Tools concept
[hooks]         # shell scripts to run — see Lifecycle
role = "..."    # role assignment — see Roles
[env.vars]      # environment variables to write
[drift]         # baseline policy — see Drift baseline
```

Everything else (`[npm]`, `[pip]`, `[cargo]`, `[git]`, `[network]`, `[telemetry]`, `[update]`, `[services]`) is an extension of one of these five.

---

## Read these in order

1. [Tools](tools.md) — what's in the registry, how tools are resolved
2. [Lifecycle](lifecycle.md) — exact ordering of setup steps
3. [Roles & inheritance](roles-and-inheritance.md) — how role merging actually works
4. [Hooks execution](hooks-execution.md) — when hooks run, what env they see
5. [Drift baseline](drift-baseline.md) — what Jarvy snapshots and why
