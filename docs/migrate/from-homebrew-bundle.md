---
title: "Migrate: Homebrew Bundle (Brewfile) → Jarvy"
description: "Step-by-step migration from a Brewfile to jarvy.toml. Map formulae and casks, preserve taps, and ship cross-platform support to teammates on Linux and Windows."
tags:
  - migrate
  - package-manager
  - homebrew

---

# Migrating from Homebrew Bundle to Jarvy

If your team has a `Brewfile` and you're already happy with it, the question is: do you have any non-macOS contributors? If yes, Jarvy gets you the same ergonomics on Linux and Windows from the same config file. If no, the migration is mostly about the extras Jarvy adds — drift detection, hooks, roles.

This guide assumes you have a working `Brewfile`. By the end you'll have a `jarvy.toml` that does everything your Brewfile did, plus.

---

## At a glance

| Brewfile concept | `jarvy.toml` equivalent |
|---|---|
| `brew "node"` | `node = "20"` (or `"latest"`) under `[provisioner]` |
| `brew "node@20"` | `node = "20"` |
| `cask "docker"` | `docker = "latest"` (Jarvy picks `cask` automatically when needed) |
| `tap "homebrew/cask-fonts"` | Not needed — Jarvy resolves the right install path. Custom taps still work via `custom_install`. |
| `mas "Xcode", id: 497799835` | Not currently supported — keep `mas` separately. |
| `brew "..." link: false` | Use `[provisioner]` detail form with platform-specific overrides. |

---

## Step 1: list what's in your Brewfile

```bash
cat Brewfile
```

Categorize:

- **CLI tools** → `[provisioner]`
- **GUI apps (casks)** → `[provisioner]` (Jarvy auto-detects)
- **Mac App Store (`mas`)** → keep in `Brewfile`, run separately
- **Custom taps** → check if the tool is already in Jarvy's [registry](../concepts/tools.md); most popular formulae are.

---

## Step 2: write the equivalent `jarvy.toml`

A typical Brewfile:

```ruby title="Brewfile"
brew "git"
brew "node@20"
brew "python@3.12"
brew "docker"
cask "visual-studio-code"
cask "iterm2"
```

Becomes:

```toml title="jarvy.toml"
[provisioner]
git              = "latest"
node             = "20"
python           = "3.12"
docker           = "latest"
vscode             = "latest"
iterm2           = "latest"
```

Run `jarvy validate` to catch any tools the registry doesn't know about. For unknown ones, you have three options:

1. Check `jarvy search <name>` for the right canonical name
2. Open a PR adding it — see [Adding tools](../adding-tools.md)
3. Use a `pre_setup` hook to shell out to `brew install ...` for that single tool

---

## Step 3: replicate side effects with hooks

Brewfile's `brew "..."` is install-only. If your team also runs `brew bundle dump --force` or has a wrapper script that does post-install setup, move that logic into hooks:

```toml
[hooks]
post_setup = "make db-seed"

[hooks.node]
post_install = "npm install -g pnpm"
```

[Hooks reference →](../hooks.md)

---

## Step 4: cross-platform check (the payoff)

Have a Linux teammate (Ubuntu, Fedora, Arch) or Windows teammate clone the repo and run `jarvy setup`. Tools that are macOS-only in your Brewfile (some casks) will fail validation early — drop them or guard with platform-conditional roles.

```toml
[roles.macos-extras]
tools = ["iterm2"]   # mac-only

[roles.base]
tools = ["git", "node", "docker"]
```

Then on macOS: `jarvy setup --role macos-extras`. On Linux/Windows, default to `base`.

---

## Step 5: commit a baseline

```bash
jarvy setup
jarvy drift accept
git add jarvy.toml .jarvy/state.json
git commit -m "chore: migrate from Brewfile to jarvy.toml"
```

You can keep the `Brewfile` around during the transition, or delete it once the team is on Jarvy.

---

## What you gain

- **Cross-platform** — same config works on macOS, Linux (apt/dnf/pacman/apk), Windows (winget/Chocolatey)
- **Version requirements** — `^`, `~`, `>=` operators instead of `node@20` only
- **Drift detection** — know when a teammate's brew is on a different version
- **Hooks** — post-install configuration declared in the same file
- **Roles** — split the toolchain by job
- **Templates** — start a new project from [14 examples](../examples.md)
- **AI agents** — Jarvy ships an [MCP server](../mcp-server.md); Brewfile doesn't

## What you give up

- Mac App Store apps via `mas` — keep that separate, or skip it
- Brewfile's tight coupling to Homebrew internals — Jarvy abstracts the package manager, which is usually a feature but occasionally not
- Your existing muscle memory of `brew bundle` — you'll be running `jarvy setup` instead

---

## Migrate with AI

Paste this into Claude, ChatGPT, Cursor, or any LLM. Drop your `Brewfile` between the `<<<` and `>>>` markers.

````text title="Prompt: Brewfile → Jarvy"
You are a config translator. Convert the Homebrew Bundle Brewfile below into
a valid jarvy.toml file.

# Schema (jarvy.toml)
- Required: [provisioner] — table of registered tool names mapped to versions.
- Versions: "latest", "20", "^3.10", "~3.12", "=20.11.0".
- Detailed: tool = { version = "20", version_manager = true }
- Optional: [npm] [pip] [cargo] [hooks] [env.vars] [env.secrets]
  [git] [network] [drift] [services] [telemetry] [commands]

# Tool-name canonicalization (CRITICAL — Brewfile names often differ)
- node@20 → node = "20"
- node@18 → node = "18"
- python@3.12 → python = "3.12"
- python@3.11 → python = "3.11"
- aws-cli → awscli
- azure-cli → azure_cli
- postgresql / postgresql@15 → psql = "latest" (Jarvy registers psql, not server)
- visual-studio-code → vscode
- iterm2 → iterm2 (cask, but registered)
- nodejs → node, python3 → python, golang → go

# What does NOT translate
- tap "user/repo" → not needed; if a tool isn't registered, flag with a comment
- mas "..." (Mac App Store) → not supported by Jarvy; keep mas separately
- brew "x" link: false / args: [...] → translate to [provisioner] detailed form
  with comments noting the dropped flags

# Per-source rules
- brew "X" or brew "X@Y" → X = "latest" or X = "Y" under [provisioner]
- cask "X" → X = "latest" under [provisioner] (Jarvy auto-detects cask vs formula)
- If a brew name isn't in the canonicalization list and you're unsure whether
  it's registered, use the brew name verbatim and add a TOML comment:
  # TODO: verify "<name>" is registered (jarvy search <name>)

# Output contract
- Output ONLY the jarvy.toml content. No prose, no fence.

# INPUT
<<<
[paste your Brewfile here]
>>>
````

After the model responds, run `jarvy validate` (it will tell you which tool names need correction).

---

## See also

- [vs Homebrew Bundle](../competitors/vs-homebrew-bundle.md) — feature comparison table
- [Configuration reference](../configuration.md)
- [Tutorial: onboard a team](../tutorials/team-onboarding.md)
