---
title: "Migrate: Gitpod → Jarvy"
description: "Move from Gitpod's `.gitpod.yml` to a local Jarvy setup. Translate tasks, image config, ports, and environment variables."
tags:
  - migrate
  - cloud
  - gitpod

---

# Migrating from Gitpod to Jarvy

Gitpod runs cloud-hosted dev environments triggered from your repository. The config lives in `.gitpod.yml` and (optionally) a custom `.gitpod.Dockerfile`. The selling points are zero local setup and a fresh environment per branch. The downsides are the same as any cloud-IDE: per-hour cost, internet dependency, remote IDE friction.

If you've been using Gitpod mainly to dodge "set up your laptop" pain, Jarvy gets you the same one-command provisioning **on the laptop**, with no recurring cost.

---

## Conceptual mapping

| `.gitpod.yml` | `jarvy.toml` equivalent |
|---|---|
| `image:` (custom Dockerfile) | Not applicable — tools install on the host |
| `tasks:` (init / before / command) | `[hooks] pre_setup` and `post_setup`, `[commands]` |
| `tasks.init` | `[hooks] post_setup` (one-time setup) |
| `tasks.command` | `[commands]` block (run-on-demand) |
| `ports:` | Not applicable — native, ports are local |
| `vscode.extensions` | Commit `.vscode/extensions.json` with recommendations |
| Workspace classes (machine size) | Whatever the laptop has |
| `gp env -e VAR=value` | `[env.vars]` or `[env.secrets]` |
| Prebuilds | Not applicable — `jarvy setup` is fast enough |

---

## Step 1: read your `.gitpod.yml`

```yaml title=".gitpod.yml (before)"
image:
  file: .gitpod.Dockerfile

tasks:
  - name: install
    init: |
      npm ci
      pip install -r requirements.txt
    command: |
      npm run dev

ports:
  - port: 3000
    onOpen: open-browser
  - port: 5432
    onOpen: ignore

vscode:
  extensions:
    - dbaeumer.vscode-eslint
    - ms-python.python
```

And your `.gitpod.Dockerfile`:

```dockerfile title=".gitpod.Dockerfile"
FROM gitpod/workspace-full:latest
RUN sudo apt-get update && sudo apt-get install -y postgresql-client
RUN nvm install 20 && nvm use 20
RUN pyenv install 3.12 && pyenv global 3.12
```

---

## Step 2: write `jarvy.toml`

```toml title="jarvy.toml"
[provisioner]
node       = "20"
python     = "3.12"
psql       = "latest"   # postgres client (psql)

[npm]
# anything previously installed by hand in the Dockerfile goes here

[pip]
# from-lockfile = true if you have requirements.txt

[hooks]
post_setup = """
npm ci
pip install -r requirements.txt
"""

[commands]
dev = "npm run dev"
```

VS Code extensions go in a separate file the editor reads natively:

```json title=".vscode/extensions.json"
{
  "recommendations": [
    "dbaeumer.vscode-eslint",
    "ms-python.python"
  ]
}
```

---

## Step 3: handle Gitpod-only patterns

**`tasks.command`** runs every time the workspace starts. Locally, it becomes a `[commands]` entry that contributors run on demand:

```bash
jarvy           # interactive menu, pick "dev"
jarvy run dev   # or run by name
```

**Ports / `onOpen: open-browser`** — there's no auto-browser-open on a workstation by default, but you can add one yourself:

```toml
[hooks]
post_setup = "npm run dev & sleep 2 && (open http://localhost:3000 || xdg-open http://localhost:3000)"
```

**Environment variables / `gp env`** — Gitpod stores user-scoped secrets in its backend. Locally, decide where they live:

```toml
[env.vars]
NODE_ENV = "development"

[env.secrets]
DATABASE_URL = { env = "DATABASE_URL", required = true }
```

For per-developer secret storage, `direnv` + `.envrc.local` (gitignored) is a clean pattern.

**Prebuilds** — Gitpod's prebuilds bake the workspace ahead of time so the first push is instant. Locally there's no analog because there's no per-PR build to wait on. `jarvy setup` is seconds-fast and idempotent — re-runs skip already-installed tools.

---

## Step 4: handle the Dockerfile

If you had a custom `.gitpod.Dockerfile`, walk through every `RUN` line:

| Dockerfile line | Translation |
|---|---|
| `RUN apt-get install foo` | `foo = "latest"` in `[provisioner]` if Jarvy knows the tool, else use a `pre_setup` hook |
| `RUN nvm install 20` | `node = "20"` (with `version_manager = true` if you want nvm specifically) |
| `RUN pip install something` | `[pip]` block with the package |
| `RUN curl ... \| bash` | `pre_setup` hook with the same command, or contribute the tool to Jarvy's [registry](../adding-tools.md) |

Keep the Dockerfile around if you also use Gitpod for ephemeral PR previews; otherwise delete it once you've migrated.

---

## Step 5: ship the README change

```markdown title="README.md"
## Local development

```bash
curl -fsSL https://raw.githubusercontent.com/bearbinary/jarvy/main/dist/scripts/install.sh | bash
jarvy setup
```

You'll have the same toolchain as the Gitpod workspace, running natively.
```

---

## What you gain

- **No per-hour bill** — even Gitpod's free tier caps; teams above the cap see the wallet hit
- **Native performance** — no remote IDE round-trip
- **Offline** — works without internet
- **Editor freedom** — Gitpod is opinionated about VS Code / JetBrains
- **Drift detection** — `jarvy drift check` shows when a teammate's machine has wandered
- **Cross-platform** — same config on macOS, Linux, Windows

## What you give up

- **Ephemeral per-PR workspaces** — Gitpod's branch-per-workspace flow has no direct local analog
- **Scale-up hardware** — workspace classes go higher than most laptops
- **Identical-environment guarantee** — container > native for strict reproducibility
- **Zero local setup** — Jarvy still needs a one-time install per laptop

---

## Migrate with AI

Paste this into Claude, ChatGPT, Cursor, or any LLM. Drop your `.gitpod.yml`
(and `.gitpod.Dockerfile` if you have one) between the `<<<` and `>>>` markers.

````text title="Prompt: Gitpod → Jarvy"
You are a config translator. Convert the Gitpod .gitpod.yml below (plus the
.gitpod.Dockerfile if provided) into a valid jarvy.toml file.

# Schema (jarvy.toml)
- Required: [provisioner] — table of registered tool names mapped to versions.
- Versions: "latest", "20", "^3.10", "~3.12", "=20.11.0".
- Detailed: tool = { version = "20", version_manager = true }
- Optional: [npm] [pip] [cargo] [hooks] [env.vars] [env.secrets]
  [git] [network] [drift] [services] [telemetry] [commands]

# Tool-name canonicalization
- nodejs → node, python3 → python, aws-cli → awscli, azure-cli → azure_cli
- postgresql / postgres → psql, visual-studio-code → vscode, golang → go

# What does NOT translate
- image: file or image: from-docker → translate Dockerfile RUN lines manually
- ports / onOpen → services bind locally; drop ports config
- vscode.extensions → user must commit .vscode/extensions.json
- workspace classes / machine types → not applicable
- prebuildConfiguration → not needed
- gp env -e (user secrets) → declare in [env.secrets]

# Per-source rules
- tasks[].init → [hooks] post_setup (concatenate multi-task inits)
- tasks[].before → [hooks] pre_setup
- tasks[].command → [commands] entry (named: dev, run, test, etc.)
- Dockerfile RUN apt-get install <tool> → <tool> under [provisioner] when known
- Dockerfile RUN nvm install / pyenv install → matching [provisioner] entry
  with version_manager = true
- Dockerfile RUN pip install / npm install -g → [pip] / [npm] block

# Output contract
- Output ONLY the jarvy.toml content. No prose, no fence.
- All hooks idempotent.

# INPUT
<<<
[paste your .gitpod.yml here, then ---DOCKERFILE--- then .gitpod.Dockerfile if any]
>>>
````

After the model responds, run `jarvy validate` and `jarvy setup --dry-run`.

---

## See also

- [vs Gitpod](../competitors/vs-gitpod.md) — feature comparison
- [Migrate from Codespaces](from-codespaces.md) — similar story, different vendor
- [Configuration reference](../configuration.md)
