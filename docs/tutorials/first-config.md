---
title: "Tutorial: Your first jarvy.toml — Jarvy"
description: "A 5-minute guided tutorial: install Jarvy, write your first jarvy.toml, provision a real Node project, and verify the environment."
tags:
  - tutorial
  - getting-started

---

# Tutorial: your first `jarvy.toml`

**Time:** ~5 minutes · **You'll need:** a terminal, an internet connection, a directory you can write to.

By the end of this tutorial you'll have:

- Jarvy installed on your laptop
- A working `jarvy.toml` describing a small Node.js + Python project
- A reproducible setup that any teammate can run with one command

We'll build a config for a hypothetical service that uses Node 20 for the API and Python 3.12 for ML scripts.

---

## 1. Install Jarvy

=== "macOS / Linux"

    ```bash
    curl -fsSL https://raw.githubusercontent.com/Cliftonz/jarvy/main/dist/scripts/install.sh | bash
    ```

=== "Homebrew"

    ```bash
    brew install jarvy
    ```

=== "Cargo"

    ```bash
    cargo install jarvy
    ```

=== "Windows (PowerShell)"

    ```powershell
    irm https://raw.githubusercontent.com/Cliftonz/jarvy/main/dist/scripts/install.ps1 | iex
    ```

Verify:

```bash
jarvy --version
```

You should see something like `jarvy 0.1.0`.

!!! tip "Already have brew, apt, or winget?"
    Jarvy uses your system's native package manager — it doesn't replace it. If `brew install node` already works for you, Jarvy makes a thin call to the same machinery.

---

## 2. Make a project directory

```bash
mkdir hello-jarvy && cd hello-jarvy
```

This is the directory we'll provision. Pretend it's a real repo — every developer who clones it will run the same commands.

---

## 3. Write `jarvy.toml`

Open `jarvy.toml` in your editor and paste:

```toml title="jarvy.toml"
[provisioner]
git    = "latest"
node   = "20"
python = "3.12"

[npm]
typescript = "^5.0"
prettier   = "latest"

[hooks.node]
post_install = "node --version && npm --version"

[env.vars]
NODE_ENV = "development"
```

What each block means:

| Block | What it does |
|---|---|
| `[provisioner]` | The CLI tools to install. Versions accept `"latest"`, major (`"20"`), or semver ranges (`">=3.10"`). |
| `[npm]` | Global npm packages installed after Node is on the machine. There are matching `[pip]` and `[cargo]` blocks. |
| `[hooks.node]` | A shell script that runs after Node is installed. Use it to print versions, run setup scripts, or seed databases. |
| `[env.vars]` | Project-scoped environment variables. Jarvy writes them to `.env` and your shell `rc`. |

---

## 4. Preview before provisioning

```bash
jarvy diff
```

`diff` compares the config to the current machine and prints what will change. No installs happen yet — this is your dry run. It should show three tools to install (or zero if you already have them) and one hook to run.

!!! tip
    `jarvy setup --dry-run` shows the same plan with the actual command sequence Jarvy will execute.

---

## 5. Provision

```bash
jarvy setup
```

You'll see colored progress lines:

```text
✓ git 2.45.0 already installed
✓ node 20.11.0 installed via brew
✓ python 3.12.1 installed via pyenv
✓ npm: typescript@5.4.3 installed
✓ npm: prettier@3.2.5 installed
✓ ran hook for node
✓ wrote .env
Setup complete in 14.3s
```

Tools that are already on the machine are detected and skipped — Jarvy is idempotent, so re-running `jarvy setup` is always safe.

---

## 6. Verify

```bash
jarvy doctor
```

`doctor` walks through every tool in your config and confirms it's installed at a satisfying version, and that the `.env` matches.

If anything is off, you'll see something like:

```text
✗ node: expected ^20, found 18.17.0
  fix: jarvy setup
```

Run the suggested fix and you're back to baseline.

---

## What just happened

You wrote a single file. Jarvy:

1. Detected your OS and picked the right package manager (Homebrew on macOS, apt/dnf on Linux, winget on Windows).
2. Resolved versions and skipped tools that already met the requirement.
3. Installed missing tools, then global npm packages.
4. Ran the post-install hook.
5. Wrote a `.env` file matching `[env.vars]`.
6. Captured a snapshot of the result so [drift detection](../drift.md) can flag changes later.

Anyone else who clones the repo runs `jarvy setup` and gets the exact same machine state.

---

## Next steps

- **Onboard your team:** [Tutorial — onboard a team in 10 minutes](team-onboarding.md)
- **Add roles:** split the config so frontend and backend devs only get what they need — [Roles guide](../roles.md)
- **Hook into bigger flows:** post-install scripts, per-tool config, services — [Hooks guide](../hooks.md)
- **Pick a template:** [14 starter `jarvy.toml` files](../templates-index.md)
- **See every option:** [Configuration reference](../configuration.md)
