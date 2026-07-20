---
title: "Recipe: Personal workstation — Jarvy as dotfiles bootstrap"
description: "Use Jarvy to provision your own laptop: shell, editor, CLI tools, git identity, and dev runtimes from a single jarvy.toml you keep in your dotfiles repo."
tags:
  - cookbook
  - personal
  - dotfiles
---

# Recipe: personal workstation (Jarvy as dotfiles bootstrap)

## Problem

You get a new laptop — or reset the one you have — and lose an afternoon
reinstalling Homebrew, `nvim`, `tmux`, your shell, `gh`, language runtimes,
and setting `git config user.email` again. You want one command to bring a
fresh machine back to the exact tools + git identity you use daily.

Jarvy's team-onboarding flow works just as well for a team of one. Keep
a `jarvy.toml` in your dotfiles repo; `jarvy setup` reconciles the machine
against it.

---

## Config

Full file at [`examples/personal-workstation/jarvy.toml`](https://github.com/Cliftonz/jarvy/blob/main/examples/personal-workstation/jarvy.toml).
Excerpt:

```toml title="~/dotfiles/jarvy.toml"
[privileges]
use_sudo = false

[provisioner]
zsh = "latest"
starship = "latest"
tmux = "latest"            # default_hook installs TPM
nvim = "latest"            # default_hook seeds ~/.config/nvim/init.lua
fzf = "latest"
zoxide = "latest"
atuin = "latest"
git = "latest"
gh = "latest"
ripgrep = "latest"
bat = "latest"
eza = "latest"
delta = "latest"
jq = "latest"

rust = { version = "stable", version_manager = true }
node = { version = "22", version_manager = true }
python = "3.12"
go = "latest"

docker = "latest"
kubectl = "latest"
k9s = "latest"

[git]
user_name  = { env = "GIT_USER_NAME",  default = "Your Name" }
user_email = { env = "GIT_USER_EMAIL", default = "you@example.com" }
default_branch = "main"
pull_rebase = true

[git.aliases]
st = "status -sb"
lg = "log --graph --oneline --decorate --all"
amend = "commit --amend --no-edit"

[commands]
reload = "jarvy setup"
drift  = "jarvy drift check"
update = "jarvy update"
```

---

## Usage

```bash
# One-time on a fresh machine
curl -fsSL https://raw.githubusercontent.com/Cliftonz/jarvy/main/dist/scripts/install.sh | sh
git clone <your-dotfiles-repo> ~/dotfiles
cd ~/dotfiles
export GIT_USER_NAME="Jane Doe"
export GIT_USER_EMAIL="jane@example.com"
jarvy setup
```

Later, after editing `jarvy.toml`:

```bash
jarvy run reload    # re-runs `jarvy setup`
jarvy run drift     # what changed on disk vs the baseline?
```

---

## Why this shape works

- **`env` with `default` in `[git]`** — you can commit this file to a
  public dotfiles repo without leaking your email. The env vars fill in
  at setup time; the default is a placeholder.
- **`version_manager = true` on `rust` / `node`** — installs via
  `rustup` / `nvm` instead of the OS package, so you can switch versions
  per-project later without fighting the system installer.
- **`tmux` and `nvim`** ship `default_hook`s that seed a minimal config
  (TPM for tmux, a starter `init.lua` for nvim) *only if none exists*.
  They never overwrite your dotfiles.
- **`[commands]`** turns muscle-memory ops into `jarvy run reload` /
  `jarvy run drift` so you don't memorize a second CLI.

---

## Extending

- Add [AI hooks](../ai-hooks.md) so Claude / Cursor share the same
  guardrails across every repo you open on this machine.
- Add [MCP registration](../mcp-registration.md) to auto-register the
  Jarvy MCP server with every AI agent CLI you install.
- If you juggle work + personal identities, use [roles](../roles.md):
  `role = "personal"` by default, `jarvy setup --role work` when you
  need the work git identity + tools.

---

## Related

- [Quickstart](../quickstart.md) — the 5-minute version
- [Roles guide](../roles.md) — split work vs personal
- [Drift](../drift.md) — detect when a hook or another tool changed
  your machine behind Jarvy's back
