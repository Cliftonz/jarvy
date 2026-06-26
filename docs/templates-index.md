---
title: "Project Templates - Jarvy"
description: "Drop-in jarvy.toml templates for common project types. Optimized for both human and AI consumption."
---

# Project Templates

Drop-in `jarvy.toml` templates for common project types. Each one is a working
config — copy it, edit versions and commands, commit.

This page is also written so an AI agent can pick the right template by stack
detection or user intent without scraping individual files.

## Decision Table

When the user describes a project, match against the **Detect** column to pick
a template. Multiple matches → pick the most specific.

| Template | Detect by | Stack | Lockfile mode | Drift tracks |
|----------|-----------|-------|---------------|--------------|
| [`node-npm`](https://github.com/Cliftonz/jarvy/tree/main/examples/node-npm) | `package-lock.json` exists, no other lockfiles | Node.js + npm | `npm ci` | `package.json`, `package-lock.json`, `.nvmrc` |
| [`node-pnpm`](https://github.com/Cliftonz/jarvy/tree/main/examples/node-pnpm) | `pnpm-lock.yaml` exists | Node.js + pnpm | `pnpm install --frozen-lockfile` | `package.json`, `pnpm-lock.yaml`, `.nvmrc`, `.npmrc` |
| [`node-bun`](https://github.com/Cliftonz/jarvy/tree/main/examples/node-bun) | `bun.lockb` or `bunfig.toml` exists | Bun runtime + node fallback | `bun install --frozen-lockfile` | `package.json`, `bun.lockb`, `bunfig.toml` |
| [`deno`](https://github.com/Cliftonz/jarvy/tree/main/examples/deno) | `deno.json` or `deno.jsonc` exists | Deno bundled toolchain | n/a (deno cache) | `deno.json`, `deno.jsonc`, `deno.lock` |
| [`python-api`](https://github.com/Cliftonz/jarvy/tree/main/examples/python-api) | `requirements.txt`, no `uv.lock` or `pyproject.toml` managed by poetry | Python + pip + venv | `pip install -r requirements.txt` | `requirements.txt`, `pyproject.toml` |
| [`python-uv`](https://github.com/Cliftonz/jarvy/tree/main/examples/python-uv) | `uv.lock` exists, or `pyproject.toml` with `[tool.uv]` | Python + uv | `uv sync --frozen` | `pyproject.toml`, `uv.lock`, `.python-version` |
| [`go-api`](https://github.com/Cliftonz/jarvy/tree/main/examples/go-api) | `go.mod` exists, HTTP service shape | Go + air + goose + golangci_lint | `go mod download` | `go.mod`, `go.sum`, `.golangci.yml` |
| [`rust-cli`](https://github.com/Cliftonz/jarvy/tree/main/examples/rust-cli) | `Cargo.toml` with single `[package]`, no `[workspace]` | Rust single-crate | n/a | `Cargo.toml`, `Cargo.lock` |
| [`rust-workspace`](https://github.com/Cliftonz/jarvy/tree/main/examples/rust-workspace) | `Cargo.toml` with `[workspace]` | Rust multi-crate + nextest + deny + machete | `cargo fetch` | `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml` |
| [`ruby-rails`](https://github.com/Cliftonz/jarvy/tree/main/examples/ruby-rails) | `Gemfile` + `config/application.rb` | Ruby on Rails + Postgres + Redis | `bundle install` | `Gemfile`, `Gemfile.lock`, `.ruby-version`, `.tool-versions` |
| [`java-spring`](https://github.com/Cliftonz/jarvy/tree/main/examples/java-spring) | `pom.xml` with `spring-boot` parent, or Spring Boot Gradle plugin | Java + OpenJDK + Maven/Gradle + Spring Boot | `mvn dependency:go-offline` or `gradle dependencies` | `pom.xml`, `build.gradle`, `gradle.properties`, `.java-version` |
| [`react-app`](https://github.com/Cliftonz/jarvy/tree/main/examples/react-app) | React + Vite project | Frontend (React/Vite) | depends on lockfile | varies |
| [`fullstack`](https://github.com/Cliftonz/jarvy/tree/main/examples/fullstack) | Mixed frontend+backend repo | Full-stack monorepo | varies | varies |
| [`k8s-platform`](https://github.com/Cliftonz/jarvy/tree/main/examples/k8s-platform) | Platform engineering repo (terraform, helm) | DevOps tooling stack | n/a | varies |

## Each Template Provides

Every template ships with the same six top-level sections so they're predictable:

1. `[provisioner]` — language runtime + universal CLI tools (git, jq, ripgrep)
2. `[npm]` / `[pip]` / `[cargo]` — language packages with lockfile mode where supported
3. `[hooks.<lang>]` — language-specific post-install (lockfile sync, completions, version probe)
4. `[hooks].post_setup` — terminal "ready to go" message + any final fetch
5. `[env.vars]` — sensible defaults (NODE_ENV, RUST_BACKTRACE, etc.)
6. `[commands]` — `run` / `test` / `build` / `lint` aliases
7. `[drift]` — drift detection enabled with language-appropriate `track_files`

## Usage Patterns

### A new contributor on a project that already has a template

```bash
# In the project root
make setup    # if the project has the bootstrap.sh + Makefile combo
# OR
jarvy setup   # if jarvy is already installed
```

### A maintainer adding Jarvy to an existing project

```bash
cd <project>
curl -fsSL https://raw.githubusercontent.com/Cliftonz/jarvy/main/examples/<template>/jarvy.toml -o jarvy.toml
$EDITOR jarvy.toml
jarvy validate
jarvy diff
git add jarvy.toml && git commit -m "feat: provision dev env via Jarvy"
```

### An AI agent generating a template for a project

1. Detect the stack (lockfiles, manifest files)
2. Pick the matching row from the decision table above
3. Fetch the template, render with the project's actual versions
4. Run `jarvy validate` to check it parses
5. Run `jarvy setup --dry-run` to show the user what will happen
6. Wait for user confirmation before any non-dry-run

## Customizing

Common edits after copying a template:

- **Pin tighter** for reproducibility: `node = "20.11.1"` instead of `node = "20"`
- **Add a role** for monorepos with split frontend/backend: see [Roles](roles.md)
- **Add proxy config** if behind a corporate firewall: see [Network & Proxy](network.md)
- **Move secrets out** of the file: use `{ env = "VAR" }` indirection

## Validation Loop

```bash
jarvy validate            # Schema + value check
jarvy diff                # See what would change
jarvy setup --dry-run     # Full plan without execution
jarvy setup               # Real run after the dry-run looks right
```

## See Also

- [Configuration Reference](configuration.md) — every field, every section
- [Adding Tools](adding-tools.md) — for tools the registry doesn't have yet
- [Tool Dependencies](tool-dependencies.md) — how install order is computed
- [For AI Agents](for-ai-agents.md) — the full AI integration guide
