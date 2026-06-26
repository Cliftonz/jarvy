---
title: "Migrate with AI — Jarvy"
description: "Use any LLM (Claude, ChatGPT, Cursor, Gemini) to translate a Brewfile, devcontainer.json, mise.toml, Vagrantfile, or other config into a working jarvy.toml. Includes tested prompt templates."
tags:
  - migrate
  - ai
  - llm

---

# Migrate with AI

Each migration guide ships a tailored prompt you can paste into Claude, ChatGPT, Cursor, or any LLM. You give it your existing config, it gives you a `jarvy.toml`.

| Source | Prompt |
|---|---|
| GitHub Codespaces / `devcontainer.json` | [→ from-codespaces guide](from-codespaces.md#migrate-with-ai) |
| DevPod / `devcontainer.json` | [→ from-devpod guide](from-devpod.md#migrate-with-ai) |
| Gitpod / `.gitpod.yml` | [→ from-gitpod guide](from-gitpod.md#migrate-with-ai) |
| VS Code Dev Containers | [→ from-dev-containers guide](from-dev-containers.md#migrate-with-ai) |
| Vagrant / `Vagrantfile` | [→ from-vagrant guide](from-vagrant.md#migrate-with-ai) |
| Homebrew Bundle / `Brewfile` | [→ from-homebrew-bundle guide](from-homebrew-bundle.md#migrate-with-ai) |
| mise / `mise.toml` | [→ from-mise guide](from-mise.md#migrate-with-ai) |
| asdf / `.tool-versions` | [→ from-asdf guide](from-asdf.md#migrate-with-ai) |
| Nix / `flake.nix` | [→ from-nix guide](from-nix.md#migrate-with-ai) |

---

## How the prompts are structured

Every prompt has the same five sections, with the per-source bits swapped in:

1. **Role** — "you are a config translator"
2. **Schema cheatsheet** — the `jarvy.toml` sections and version syntax the LLM needs to know
3. **Source-specific rules** — what translates, what drops, what gets renamed
4. **Tool-name registry hints** — common name corrections (`aws-cli` → `awscli`, `postgresql` → `psql`, `nodejs` → `node`)
5. **Output contract** — output ONLY the TOML, no prose

This structure was designed and tuned against the [migration fixture suite](https://github.com/Cliftonz/jarvy/tree/main/tests/migrate/fixtures) and validated against real `jarvy validate` runs in the [promptfoo eval harness](https://github.com/Cliftonz/jarvy/tree/main/evals/migrate).

---

## After the LLM responds

Always validate what the model gave you:

```bash
# Save the output to a file
$EDITOR jarvy.toml

# Schema check
jarvy validate

# What would happen
jarvy diff
jarvy setup --dry-run

# Provision
jarvy setup
```

If `validate` complains:

| Error | Fix |
|---|---|
| `Unknown tool: 'foo'` | Run `jarvy search foo` for the canonical name |
| `Invalid version` | Use one of: `"latest"`, `"20"`, `"^3.10"`, `"~3.12"`, `"=1.6.6"` |
| Section warnings | The validator's known-section list is conservative — feature blocks like `[npm]`, `[pip]`, `[cargo]`, `[git]`, `[network]`, `[drift]`, `[telemetry]`, `[commands]` are real; warnings are advisory |

---

## When *not* to use AI for migration

- **Single-file tool-version pin** (`.tool-versions` with two lines): faster to translate by hand than to prompt.
- **Highly customized Dockerfiles** with build-time scripts, multi-stage builds, or compiled deps: the LLM will flatten too much. Walk it manually.
- **Multi-VM Vagrantfiles**: Jarvy's model isn't 1:1; the migration deserves a thinking pass, not a one-shot.

For everything in between, the prompts get you 90% of the way; you tune the last 10%.
