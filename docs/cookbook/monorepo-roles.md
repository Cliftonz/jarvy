---
title: "Recipe: Monorepo with frontend / backend / devops roles — Jarvy"
description: "Split a monorepo's toolchain by role so frontend developers don't install kubectl and backend developers don't install Cypress."
tags:
  - cookbook
  - roles
  - monorepo
---

# Recipe: monorepo with frontend / backend / devops roles

## Problem

Your monorepo houses a React frontend, a Go API, and a Kubernetes deployment manifest. Every contributor runs `jarvy setup`, but you don't want frontend devs installing `kubectl` and `helm`, and you don't want platform engineers waiting through a `pnpm install` of 800 packages they'll never use.

---

## Config

```toml title="jarvy.toml"
role = "frontend"   # default for fresh checkouts

[roles.base]
description = "Tools every contributor needs"
tools = ["git", "docker", "make"]

[roles.frontend]
description = "React frontend developers"
extends = "base"
tools = ["node", "pnpm"]

[roles.frontend.tools]
node = "20"

[roles.backend]
description = "Go API developers"
extends = "base"
tools = ["go", "golangci-lint", "air", "psql"]

[roles.backend.tools]
go = "1.22"

[roles.devops]
description = "Platform / infrastructure"
extends = "base"
tools = ["kubectl", "helm", "terraform", "kustomize", "stern"]

[roles.fullstack]
description = "People who do both"
extends = "frontend"
tools = ["go", "golangci-lint"]

# Hooks scoped per-role
[hooks.frontend.node]
post_install = "pnpm install"

[hooks.backend.go]
post_install = "go mod download"

[commands]
dev   = "make dev"
test  = "make test"
build = "make build"
```

---

## Why it works

| Pattern | Effect |
|---|---|
| `role = "frontend"` at top | Default for new checkouts — no env vars, no shell config |
| `extends = "base"` | Each role automatically gets git, docker, make |
| `[roles.frontend.tools]` override | Pins node to 20 just for frontend devs |
| `extends = "frontend"` on fullstack | Inheritance chains cleanly; up to 5 levels deep |
| `[hooks.<role>.<tool>]` | Hook only fires if that role's pulling in that tool |

Each contributor either:

- Sticks with the default (`jarvy setup`)
- Switches per-run (`jarvy setup --role devops`)
- Sets a per-checkout override (commit a different `role = ...` to a personal branch... but that's smelly — better to use `--role`)

---

## Variations

**Multiple roles for "I do everything" people:**

```toml
role = ["frontend", "backend"]
```

Last entry wins for conflicts.

**Optional roles (opt-in, never default):**

Don't set a default `role`. Each contributor must use `--role <name>`. Stricter, but no implicit toolchain.

**Roles for different OSes:**

```toml
[roles.macos-extras]
tools = ["iterm2", "raycast"]
```

Run on macOS only: `jarvy setup --role frontend --role macos-extras`.

**Inspecting what a role would install:**

```bash
jarvy roles show frontend --resolved      # full tool list including inherited
jarvy roles show frontend --inheritance   # the extends chain
jarvy roles diff frontend backend         # what changes between two roles
```

---

## Caveats

- **Roles aren't environments.** `[env.vars]` is global — same env for every role. If you need per-role env, use a `pre_setup` hook that branches on `$JARVY_ROLE`.
- **Don't over-engineer.** If your team is six people and everyone needs the same tools, skip roles entirely. A flat `[provisioner]` is simpler and reads better.
- **CI uses one role.** Pick one for your default CI matrix (typically `base` or whichever role your CI runs against), and add a separate matrix entry per role you want to test.

---

## See also

- [Roles guide](../roles.md) — full TOML reference
- [Roles & inheritance concept](../concepts/roles-and-inheritance.md) — precedence rules and resolution model
