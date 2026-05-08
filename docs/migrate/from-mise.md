---
title: "Migrate: mise → Jarvy"
description: "Side-by-side migration from a `.tool-versions` or `mise.toml` to `jarvy.toml` — keep the runtime version pinning, gain native install, drift detection, and team roles."
tags:
  - migrate
  - runtime
  - mise

---

# Migrating from mise to Jarvy

mise (formerly rtx, the spiritual successor to asdf) is great at one job: pinning **language runtime versions** per project. Jarvy is broader — it manages the entire dev toolchain (CLIs, GUIs, language packages, hooks, env vars) across all three OSes — but it still pins runtimes when you want it to.

If you're using mise mostly for runtime version management, the migration is short. If you're using mise's plugins for non-runtime tools, you'll move those into Jarvy's `[provisioner]` instead.

---

## Conceptual mapping

| mise concept | `jarvy.toml` equivalent |
|---|---|
| `[tools]` in `mise.toml` | `[provisioner]` |
| `node = "20"` | `node = "20"` |
| `python = "3.12"` | `python = "3.12"` |
| `[env]` | `[env.vars]` |
| Plugin-installed tools (`terraform`, `kubectl`) | `[provisioner]` — they're in Jarvy's [registry](../concepts/tools.md) natively |
| Runtime version manager (`asdf`-style) | `version_manager = true` in detailed form |
| `_tasks` (mise's task runner) | `[hooks]` and `[commands]` |

---

## Step 1: dump your mise state

```bash
mise current
mise ls
cat mise.toml 2>/dev/null || cat .tool-versions
```

A typical `mise.toml`:

```toml title="mise.toml (before)"
[tools]
node = "20.11.0"
python = "3.12"
terraform = "1.6.6"

[env]
NODE_ENV = "development"
DATABASE_URL = "postgres://localhost/myapp"
```

---

## Step 2: write `jarvy.toml`

```toml title="jarvy.toml (after)"
[provisioner]
node      = "=20.11.0"
python    = "3.12"
terraform = "=1.6.6"

[env.vars]
NODE_ENV     = "development"
DATABASE_URL = "postgres://localhost/myapp"
```

A few notes:

- mise's `node = "20.11.0"` means *exactly that version*. Jarvy's `node = "20.11.0"` is shorthand for "major 20"; use `=20.11.0` for an exact match.
- mise installs runtimes into a managed shim directory. Jarvy uses the system package manager by default, but you can opt into `version_manager = true` to keep nvm/pyenv-style isolation.

---

## Step 3: keep version-manager isolation if you want it

If your team relies on mise's per-project shim, opt into Jarvy's version-manager mode:

```toml
[provisioner]
node   = { version = "20.11.0", version_manager = true }   # uses nvm
python = { version = "3.12", version_manager = true }      # uses pyenv
```

Otherwise, Jarvy installs Node via Homebrew (or apt, or winget) and trusts your shell's PATH to resolve it. Both are valid; the team picks one.

---

## Step 4: move tasks to commands or hooks

mise tasks:

```toml title="mise.toml"
[tasks.test]
run = "pytest"
```

Jarvy equivalents:

```toml title="jarvy.toml"
# Long-form task → hook
[hooks]
post_setup = "pytest --collect-only > /dev/null"   # smoke test after setup

# Project commands → exposed in interactive menu
[commands]
test = "pytest"
lint = "ruff check ."
build = "python -m build"
```

`[commands]` shows up in `jarvy` (with no subcommand) as a menu — handy for new contributors.

---

## Step 5: shut off mise

After confirming `jarvy setup && jarvy doctor` is green:

```bash
mise deactivate              # remove the mise shim from your shell
rm mise.toml .tool-versions  # if you're committed
```

Some teams keep `.tool-versions` around as a fallback for editor integrations that read it directly (like the JetBrains "asdf" plugin). It's redundant but harmless.

---

## What you gain over mise

- **Beyond runtimes** — manage CLI tools (docker, kubectl, terraform), GUI apps (casks), language packages (`[npm]`, `[pip]`, `[cargo]`)
- **Hooks** — declarative post-install setup
- **Roles** — split the toolchain by job
- **Drift detection** — `mise` doesn't snapshot
- **Cross-platform parity** — mise is Unix-first; Jarvy is first-class on Windows
- **MCP server** — AI agents can install and configure tools

## What mise still does better

- **Per-shell version switching** — `cd`-into-a-directory-and-the-Node-version-changes is mise's killer feature. Jarvy doesn't replace shell shims; if you need that workflow, keep mise around for runtimes only and use Jarvy for everything else.
- **Plugin ecosystem** — mise has a long tail of community plugins; Jarvy's registry is broader on common tools but doesn't cover every obscure CLI.

---

## Hybrid: keep mise for runtimes, Jarvy for everything else

This is a perfectly reasonable end state:

```toml title="jarvy.toml"
[provisioner]
mise = "latest"   # install mise itself
git, docker, kubectl, terraform = ...

[hooks.mise]
post_install = "mise install"   # delegate runtime install to mise
```

Jarvy provisions the toolchain; mise handles per-project runtime switching. They compose cleanly.

---

## Migrate with AI

Paste this into Claude, ChatGPT, Cursor, or any LLM. Drop your `mise.toml` (or `.tool-versions`) between the `<<<` and `>>>` markers.

````text title="Prompt: mise → Jarvy"
You are a config translator. Convert the mise.toml (or .tool-versions) below
into a valid jarvy.toml file.

# Schema (jarvy.toml)
- Required: [provisioner] — table of registered tool names mapped to versions.
- Versions: "latest", "20", "^3.10", "~3.12", "=20.11.0".
- Detailed: tool = { version = "20", version_manager = true }
- Optional: [npm] [pip] [cargo] [hooks] [env.vars] [env.secrets]
  [git] [network] [drift] [services] [telemetry] [commands]

# Tool-name canonicalization
- nodejs → node
- python3 → python
- ruby > 2 → ruby
- golang → go

# Critical version semantics difference
- mise's `node = "20.11.0"` means EXACTLY that version.
- Jarvy's `node = "20.11.0"` is shorthand for "major 20".
- Always translate exact mise versions to Jarvy's exact form: "=20.11.0".

# What does NOT translate
- Plugin URLs / custom plugin sources → use registered tool names; if missing,
  flag with TOML comment "# TODO: contribute <name> to Jarvy registry"

# Per-source rules
- [tools] section → [provisioner] section
- [env] section → [env.vars] section
- mise tasks ([tasks.X] run = "...") → [commands] entry where the key is the
  task name and value is the run string
- If user wanted shim isolation behavior of mise, set version_manager = true
  on each runtime tool

# Output contract
- Output ONLY the jarvy.toml content. No prose, no fence.

# INPUT
<<<
[paste your mise.toml or .tool-versions here]
>>>
````

After the model responds, run `jarvy validate` and `jarvy setup --dry-run`.

---

## See also

- [vs mise](../competitors/vs-mise.md) — feature comparison
- [Configuration reference](../configuration.md)
- [Tutorial: your first jarvy.toml](../tutorials/first-config.md)
