---
title: "Migration guides — Jarvy"
description: "Switch to Jarvy from Codespaces, DevPod, Gitpod, Dev Containers, Vagrant, Homebrew Bundle, mise, asdf, or Nix. Step-by-step guides with conceptual mappings."
tags:
  - migrate

---

# Migration guides

You don't have to start from scratch. Pick the tool you're moving away from — each guide is a one-page walkthrough with conceptual mappings, a worked example, and an honest "what you give up" section.

!!! tip "Use AI to do the translation"
    Every guide ships with a tested LLM prompt that takes your existing config and outputs a working `jarvy.toml`. See [Migrate with AI](migrate-with-ai.md) for the index of prompts.

---

## Cloud / container dev environments

For teams paying for cloud-hosted environments or running a container per project.

| You have | Migration guide |
|---|---|
| GitHub Codespaces | [→ Migrating from Codespaces](from-codespaces.md) |
| DevPod | [→ Migrating from DevPod](from-devpod.md) |
| Gitpod | [→ Migrating from Gitpod](from-gitpod.md) |
| VS Code Dev Containers (`devcontainer.json`) | [→ Migrating from Dev Containers](from-dev-containers.md) |
| Vagrant | [→ Migrating from Vagrant](from-vagrant.md) |

---

## Toolchain managers

For teams already managing tools declaratively, just with a different tool.

| You have | Migration guide |
|---|---|
| Homebrew Bundle (`Brewfile`) | [→ Migrating from Homebrew Bundle](from-homebrew-bundle.md) |
| mise (`mise.toml`, `.tool-versions`) | [→ Migrating from mise](from-mise.md) |
| asdf (`.tool-versions`) | [→ Migrating from asdf](from-asdf.md) |
| Nix (`flake.nix`, `shell.nix`) | [→ Migrating from Nix](from-nix.md) |

Don't see your tool? Open a [discussion](https://github.com/bearbinary/jarvy/discussions) or check the [comparison pages](../competitors/competitive-analysis.md) for feature differences.

---

## What every migration guide covers

1. **Conceptual mapping** — your tool's primitives, translated to `jarvy.toml`
2. **Worked example** — a realistic config translated side-by-side
3. **Edge cases** — what doesn't translate cleanly, and how to handle it
4. **What you gain** — features Jarvy adds
5. **What you give up** — where the old tool was better
6. **Hybrid options** — when to keep both running

The honest assessment is the point. If Jarvy isn't a clean win for your team, the migration guide will say so.

---

## Common patterns across migrations

A few translations show up in nearly every guide:

| Source primitive | `jarvy.toml` |
|---|---|
| Tool / language pin | `[provisioner]` |
| Post-install script (one-time) | `[hooks] post_setup` |
| Per-tool post-install | `[hooks.<tool>] post_install` |
| Environment variable | `[env.vars]` |
| Required secret | `[env.secrets] FOO = { env = "FOO", required = true }` |
| Project task / command | `[commands]` |
| Editor extensions | `.vscode/extensions.json` (Jarvy doesn't manage editor plugins) |
| Reproducibility | `[drift] version_policy = "exact"` + commit `.jarvy/state.json` |

If the migration guide for your specific tool doesn't exist yet, these mappings will get you 80% of the way.
