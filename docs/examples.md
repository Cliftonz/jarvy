---
title: "Examples gallery — Jarvy"
description: "Drop-in jarvy.toml templates for Node, Python, Go, Rust, Ruby, Java, fullstack, and Kubernetes platform engineering. Copy, edit, commit."
tags:
  - examples
  - templates

---

# Examples gallery

Every template is a real, runnable `jarvy.toml` you can copy into your repo's root and ship today. Pick the closest match, edit the versions, commit.

```bash
# Copy from the repo
curl -fsSL https://raw.githubusercontent.com/Cliftonz/jarvy/main/examples/<template>/jarvy.toml \
  -o jarvy.toml
$EDITOR jarvy.toml
jarvy setup
```

Or use the CLI:

```bash
jarvy init --template node-pnpm
```

---

## Language-specific

| Template | Stack | Best for |
|---|---|---|
| [`node-npm`](https://github.com/Cliftonz/jarvy/tree/main/examples/node-npm) | Node.js · npm | Vanilla Node services, libraries, CLI tools |
| [`node-pnpm`](https://github.com/Cliftonz/jarvy/tree/main/examples/node-pnpm) | Node.js · pnpm | Monorepos, faster installs, strict dependency resolution |
| [`node-bun`](https://github.com/Cliftonz/jarvy/tree/main/examples/node-bun) | Bun runtime | Edge functions, scripts, fast TypeScript builds |
| [`deno`](https://github.com/Cliftonz/jarvy/tree/main/examples/deno) | Deno | Secure-by-default scripts and APIs |
| [`python-api`](https://github.com/Cliftonz/jarvy/tree/main/examples/python-api) | Python · pip · venv | FastAPI, Django, Flask APIs |
| [`python-uv`](https://github.com/Cliftonz/jarvy/tree/main/examples/python-uv) | Python · uv | Modern, fast pip replacement; reproducible installs |
| [`go-api`](https://github.com/Cliftonz/jarvy/tree/main/examples/go-api) | Go · air · goose · golangci-lint | Go HTTP services with hot reload and migrations |
| [`rust-cli`](https://github.com/Cliftonz/jarvy/tree/main/examples/rust-cli) | Rust single-crate | CLI tools, focused libraries |
| [`rust-workspace`](https://github.com/Cliftonz/jarvy/tree/main/examples/rust-workspace) | Rust workspace | Multi-crate Cargo workspaces |
| [`ruby-rails`](https://github.com/Cliftonz/jarvy/tree/main/examples/ruby-rails) | Ruby on Rails · Postgres · Redis | Rails apps with full local services |
| [`java-spring`](https://github.com/Cliftonz/jarvy/tree/main/examples/java-spring) | Java · Spring Boot | Maven or Gradle Spring services |

---

## Multi-service

| Template | Stack |
|---|---|
| [`react-app`](https://github.com/Cliftonz/jarvy/tree/main/examples/react-app) | React + Vite frontend |
| [`fullstack`](https://github.com/Cliftonz/jarvy/tree/main/examples/fullstack) | Frontend + backend + database, ready for `docker compose` |
| [`k8s-platform`](https://github.com/Cliftonz/jarvy/tree/main/examples/k8s-platform) | Platform engineering: kubectl, helm, terraform, k9s, … |

---

## What every template includes

```toml
[provisioner]   # language runtime + universal CLI tools
[npm]/[pip]/[cargo]   # language-specific packages
[hooks]         # lockfile sync, completions, project setup
[env.vars]      # sensible defaults (NODE_ENV, PYTHONUNBUFFERED, …)
[commands]      # run / test / build / lint aliases for the interactive menu
[drift]         # drift detection enabled with appropriate track_files
```

---

## Customizing a template

Common edits:

| Edit | When |
|---|---|
| Pin tighter — `node = "20.11.1"` instead of `node = "20"` | Reproducibility for releases or audits |
| Add roles — `role = ["frontend", "devops"]` | Mixed teams sharing one repo |
| Add `[network]` block | Behind a corporate proxy |
| Move secrets to `{ env = "VAR" }` | Anything sensitive — never hardcode |
| Tighten `[drift] version_policy` | Lock down patch versions for security-sensitive projects |

[Configuration reference →](configuration.md)

---

## Validate before committing

```bash
jarvy validate         # schema + value check
jarvy diff             # what would change on this machine
jarvy setup --dry-run  # full plan, no execution
```

---

## Don't see your stack?

Open a [discussion](https://github.com/Cliftonz/jarvy/discussions) or contribute a template via PR — see the [examples README](https://github.com/Cliftonz/jarvy/blob/main/examples/README.md) for the contribution flow.
