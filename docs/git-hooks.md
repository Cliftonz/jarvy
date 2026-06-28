---
title: "Git hooks — Jarvy"
description: "Install and manage pre-commit (and future husky / lefthook) framework hooks from jarvy.toml. Auto-install during setup, status checks, and CI considerations."
tags:
  - guides
  - git
---

# Git hooks

`jarvy hooks` installs and manages git pre-commit framework integrations directly from `jarvy.toml`. The intent: "every contributor gets the same lint / format / type-check gates without anyone running `pre-commit install` by hand."

Today the [`pre-commit`](https://pre-commit.com) framework is fully supported. [Husky](https://typicode.github.io/husky/) and [Lefthook](https://github.com/evilmartians/lefthook) are recognized by auto-detection but their handlers are stubbed — the CLI reports a clear "framework configured but not yet supported" error rather than silently no-op-ing.

---

## Why `[git_hooks]` and not `[hooks]`

`[hooks]` is already taken by `jarvy setup` for `pre_setup` / `post_install` / `post_setup` shell scripts (PRD-003). Mixing git hooks into that block would tangle two unrelated lifecycles, so the new section is its own top-level `[git_hooks]` block. They can be used independently — git hooks without setup hooks, or vice versa.

---

## Configuration

Minimal:

```toml
[git_hooks]
```

The block's presence is the opt-in. With nothing else set, `jarvy setup` auto-detects a framework (looks for `.pre-commit-config.yaml`, `.husky/`, or `lefthook.yml`) and installs it.

Pinned + explicit:

```toml
[git_hooks]
enabled = true                    # default true
framework = "pre-commit"          # pre-commit | husky | lefthook | native
auto_install = true               # install during `jarvy setup` (default true)
auto_update = false               # run `pre-commit autoupdate` after install (default false)
run_after_install = false         # run hooks once against the whole tree (default false)
allow_remote = false              # remote-config trust gate (default false)

[git_hooks.pre_commit]
version = "3.6.0"                          # pin pre-commit framework version
config = ".pre-commit-config.yaml"         # path to framework config (default)
install_hooks = true                       # `--install-hooks` (warms hook envs eagerly)
```

| Knob | Default | Why this default |
|------|---------|------------------|
| `enabled` | `true` | The block's presence implies the user wants hooks; set `false` to declare-but-disable |
| `auto_install` | `true` | "Add `[git_hooks]` and forget" is the target UX |
| `auto_update` | `false` | `autoupdate` can rev pinned versions across the whole team unexpectedly |
| `run_after_install` | `false` | First-run can be slow and surfaces unrelated lint debt in the install transcript |
| `allow_remote` | `false` | Mirrors `[packages] allow_remote` — remote configs may NOT install hooks without explicit source opt-in |
| `pre_commit.install_hooks` | `true` | First-commit latency surprise is a worse UX than the extra install-time cost |

---

## Commands

```bash
jarvy hooks install            # install framework into .git/hooks/
jarvy hooks update             # pre-commit autoupdate + reinstall
jarvy hooks status             # framework + installed?  + hook count
jarvy hooks list               # parse .pre-commit-config.yaml, print hooks grouped by repo
jarvy hooks run                # run against staged changes
jarvy hooks run --all-files    # run against entire tree
jarvy hooks run --hook black   # run a single hook by id
jarvy hooks uninstall          # pre-commit uninstall
```

`jarvy setup` auto-runs `jarvy hooks install` between the git-config phase and the AI-hooks phase, gated on `[git_hooks].auto_install`.

---

## Status output

```
$ jarvy hooks status
Git Hooks Status
================
Framework:    pre-commit
Installed:    yes
Config:       .pre-commit-config.yaml
Hook count:   7
```

`Hook count` parses `.pre-commit-config.yaml` directly — no subprocess invocation, so this works even when the `pre-commit` CLI itself isn't installed yet.

---

## Trust boundary

`jarvy setup --from <url>` can fetch a remote `jarvy.toml`. Pre-commit configs reference hook repos by URL + revision — installing those hooks fetches and executes arbitrary code at commit time. To prevent a friendly-looking remote config from silently landing arbitrary git hooks on the consuming machine, remote configs are refused at the hook-install gate unless `[git_hooks] allow_remote = true` is set in the SOURCE config.

This mirrors `[packages] allow_remote` and `[ai_hooks] allow_custom_commands`. The policy travels with the file: setting `allow_remote = true` in your own local config does NOT broaden trust for files you fetch from elsewhere.

When a remote config is refused, `jarvy setup` logs a `git_hooks.remote_refused` event and prints a one-line warning, then continues with the rest of the run.

---

## CI

In CI, `jarvy hooks install` is usually unnecessary — CI runs the lint / format checks directly via `pre-commit run --all-files` rather than installing hooks into a transient `.git/hooks/` directory that's discarded with the runner.

`jarvy setup` auto-detects CI via `jarvy ci-info` and the existing sandbox detection. The git-hooks phase still runs in CI by default; opt out with:

```toml
[git_hooks]
auto_install = false             # in jarvy.toml
```

Or per-run:

```bash
jarvy setup --no-hooks           # skips ALL setup hooks AND git hooks
```

---

## Troubleshooting

- **`hook framework 'pre-commit' is not installed`** — install pre-commit first: `pip install pre-commit` or add it to your `[provisioner]` block (recommended: install it as part of `jarvy setup` so the order is correct).
- **`pre-commit config not found at .pre-commit-config.yaml`** — create the config first; Jarvy doesn't synthesize one for you. See the [pre-commit docs](https://pre-commit.com/#2-add-a-pre-commit-configuration) for the format.
- **`not inside a git repository`** — git hooks need `.git/` to live. Run `git init` first.
- **`framework 'husky' is configured but not yet supported`** — only pre-commit ships today. File an issue if you need husky / lefthook prioritized.
- **Hooks run twice during `jarvy setup`** — you probably have `run_after_install = true` AND a separate `post_setup` hook that also runs them. Pick one.

---

## What's next

- husky framework support (npm / yarn / pnpm workflows)
- lefthook framework support (Go / Ruby / Rust workflows that prefer it over pre-commit)
- Per-stage hook configuration (`commit-msg`, `pre-push`, etc.)

Track progress under `prd/048-pre-commit-hook-installation.md`.

---

## Related

- [Configuration reference](configuration.md) — full `[git_hooks]` schema
- [Hooks guide](hooks.md) — `jarvy setup` lifecycle hooks (NOT git hooks)
- [pre-commit official docs](https://pre-commit.com)
