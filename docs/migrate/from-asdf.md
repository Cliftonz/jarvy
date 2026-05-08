---
title: "Migrate: asdf → Jarvy"
description: "Move from asdf's `.tool-versions` and plugin model to a unified `jarvy.toml`. Keep runtime version pinning, gain CLI-tool management and drift detection."
tags:
  - migrate
  - runtime
  - asdf

---

# Migrating from asdf to Jarvy

asdf is a runtime-only tool: install plugins for each language, list versions in `.tool-versions`, switch versions per-directory. Jarvy is broader — it manages the entire dev toolchain, not just runtimes — and it ships with a curated tool registry instead of community plugins.

The asdf → Jarvy migration is mostly mechanical: read `.tool-versions`, write `[provisioner]`. Where asdf had a plugin you used for a CLI tool (terraform, kubectl), Jarvy has the tool natively in its registry.

---

## Conceptual mapping

| asdf concept | `jarvy.toml` equivalent |
|---|---|
| `.tool-versions` | `[provisioner]` |
| `nodejs 20.11.0` | `node = "20.11.0"` |
| `python 3.12.1` | `python = "3.12.1"` |
| `asdf install` | `jarvy setup` |
| `asdf plugin add` | Not needed — tools are pre-registered. Custom installers via `custom_install`. |
| Per-directory shim | `version_manager = true` (uses nvm/pyenv-style isolation) or system PATH |

---

## Step 1: dump asdf state

```bash
cat .tool-versions
asdf plugin list
```

Typical `.tool-versions`:

```text title=".tool-versions"
nodejs 20.11.0
python 3.12.1
ruby 3.3.0
terraform 1.6.6
```

---

## Step 2: write `jarvy.toml`

```toml title="jarvy.toml"
[provisioner]
node      = "20.11.0"
python    = "3.12.1"
ruby      = "3.3.0"
terraform = "1.6.6"
```

Note: Jarvy uses `node`, not `nodejs`. Most names match asdf plugin names; check `jarvy search <name>` if unsure.

---

## Step 3: opt into version-manager isolation (optional)

asdf's per-shell shim is convenient. If you want to keep that exact behavior, opt Jarvy in:

```toml
[provisioner]
node = { version = "20.11.0", version_manager = true }   # uses nvm
ruby = { version = "3.3.0", version_manager = true }     # uses rbenv
```

If you'd rather have a single system-wide install (faster, simpler, no shim layer), leave `version_manager` off.

---

## Step 4: shut off asdf

```bash
# Confirm Jarvy is healthy first
jarvy setup
jarvy doctor

# Then remove asdf shims from shell rc
asdf reshim   # or remove the asdf source line entirely
rm .tool-versions
```

Some teams keep `.tool-versions` around because IDEs read it directly. It's harmless.

---

## What you gain

- **Beyond runtimes** — manage docker, kubectl, terraform, aws-cli, postgres, redis, … all in one file
- **Hooks** — declarative post-install setup, not separate `asdf-postinstall` plugins
- **Roles** — split the toolchain by job
- **Drift detection** — asdf doesn't snapshot
- **No plugin curation** — Jarvy's registry is centrally maintained; asdf plugins are community-maintained with varying quality
- **MCP server** — AI agents can install and configure tools

## What asdf still does better

- **Per-shell version switching** — asdf's killer feature; Jarvy doesn't replace shell shims out of the box. If you need this, set `version_manager = true` (delegates to nvm/rbenv) or keep asdf for runtimes only.
- **Long tail of community plugins** — for an obscure tool that asdf has a plugin for and Jarvy doesn't, you'll need to either contribute it ([Adding tools](../adding-tools.md)) or use a `pre_setup` hook to install it.

---

## Hybrid: keep asdf for runtimes, Jarvy for everything else

Reasonable end state:

```toml title="jarvy.toml"
[provisioner]
asdf    = "latest"
docker  = "latest"
kubectl = "latest"

[hooks.asdf]
post_install = "asdf install"   # delegate runtime install
```

Jarvy provisions the toolchain. asdf does what it's good at — per-project runtime switching.

---

## Migrate with AI

Paste this into Claude, ChatGPT, Cursor, or any LLM. Drop your `.tool-versions` between the `<<<` and `>>>` markers.

````text title="Prompt: asdf → Jarvy"
You are a config translator. Convert the asdf .tool-versions file below into
a valid jarvy.toml file.

# Schema (jarvy.toml)
- Required: [provisioner] — table of registered tool names mapped to versions.
- Versions: "latest", "20", "^3.10", "~3.12", "=20.11.0".
- Detailed: tool = { version = "20", version_manager = true }
- Optional: [npm] [pip] [cargo] [hooks] [env.vars] [env.secrets]
  [git] [network] [drift] [services] [telemetry] [commands]

# Tool-name canonicalization (CRITICAL — asdf plugins often use long names)
- nodejs → node
- python → python (no rename, but version pin matters — see below)
- ruby → ruby
- golang → go
- terraform → terraform
- kubectl → kubectl

# Critical version semantics
- asdf's "20.11.0" means EXACTLY that version (it's installed via plugin).
- Jarvy's "20.11.0" is shorthand for "major 20".
- Translate every asdf line as exact: "=20.11.0".

# What does NOT translate
- Plugin install commands (asdf plugin add) → not needed; tools are registered
- system fallback ("system") → omit; user's system tool wins via PATH

# Per-source rules
- One line per tool: "name version" → name = "=version" under [provisioner]
- Multi-version lines (asdf supports "node 20.11.0 18.17.0") → take the first
  (preferred) version only; flag others with comment

# Output contract
- Output ONLY the jarvy.toml content. No prose, no fence.

# INPUT
<<<
[paste your .tool-versions here]
>>>
````

After the model responds, run `jarvy validate` and `jarvy setup --dry-run`.

---

## See also

- [vs asdf](../competitors/vs-asdf.md) — feature comparison
- [Configuration reference](../configuration.md)
- [Tutorial: your first jarvy.toml](../tutorials/first-config.md)
