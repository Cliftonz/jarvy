---
title: "Services — Jarvy"
description: "Manage Docker Compose, Tilt, and standalone container services from jarvy.toml. Auto-start, lifecycle commands, and CI considerations."
tags:
  - guides
  - services
---

# Services

Jarvy can start, stop, and monitor the supporting services your project depends on — Postgres, Redis, RabbitMQ, Tilt clusters, anything described by Docker Compose. Configuration lives in the `[services]` block of `jarvy.toml`; lifecycle is driven by `jarvy services` and (optionally) `jarvy setup`.

---

## When to use this

If a contributor reads your README and sees "make sure Postgres is running on :5432, run `docker compose up redis tempo`, then `npm run dev`", `[services]` is where that disappears.

Skip it if your dev loop is single-binary or if all infra runs in production-like Kubernetes via Tilt only (Tilt's own `Tiltfile` already covers that case — see the Tilt section below for the integration shape).

---

## Backends

| Backend | Use when | Configuration key |
|---------|----------|-------------------|
| **Docker Compose** | You already have a `docker-compose.yml` | `compose_file = "docker-compose.yml"` |
| **Tilt** | Your dev loop is Kubernetes-shaped | `tilt = true` (uses repo `Tiltfile`) |
| **Inline service blocks** | A single container image, no compose | `[services.<name>] image = "..." port = 5432` |

Jarvy delegates to the backend's native CLI — there is no Jarvy-specific orchestrator. If `docker compose up` works at the shell, `jarvy services start` works.

---

## Configuration

### Docker Compose

```toml
[services]
enabled = true
auto_start = true                  # run on `jarvy setup`
compose_file = "docker-compose.yml" # default; omit if standard
profiles = ["dev", "observability"] # optional Compose profiles
```

`jarvy services start` runs `docker compose --file <compose_file> --profile <p> up -d`. Stop / status / restart mirror the corresponding Compose verbs.

### Tilt

```toml
[services]
enabled = true
auto_start = false                 # Tilt UI is interactive; don't background it
tilt = true                        # uses ./Tiltfile
```

`jarvy services start` runs `tilt up`. `jarvy services stop` runs `tilt down`. The Tilt UI (`http://localhost:10350`) replaces `jarvy services status` while Tilt is running.

### Inline service blocks

For a one-image dependency where Compose is overkill:

```toml
[services]
enabled = true
auto_start = true

[services.postgres]
image = "postgres:16"
port = 5432
env = { POSTGRES_PASSWORD = "dev" }

[services.redis]
image = "redis:7"
port = 6379
```

Jarvy synthesizes a `docker run -d --name <name> -p <port>:<port> ...` per service. Best for small dev setups; switch to a real `docker-compose.yml` once you have more than 2-3 services.

---

## Commands

```bash
jarvy services start       # start everything declared in [services]
jarvy services stop        # stop without removing volumes
jarvy services restart     # stop then start
jarvy services status      # which services are up, port mappings, health
jarvy services logs <svc>  # tail logs for a single service
```

All commands honor `--file <path>` to point at an alternate `jarvy.toml`, useful in monorepos.

---

## Auto-start on `jarvy setup`

```toml
[services]
auto_start = true
```

Adds a `jarvy services start` step to the end of `jarvy setup`. The intended workflow: a new contributor runs `./scripts/bootstrap.sh`, comes back to a coffee, and finds a fully running dev stack — tools installed, services up, ready to `npm run dev`.

**Auto-disable in CI.** Auto-start is suppressed when `jarvy ci-info` reports a CI environment, because pulling and starting a Postgres container during PR CI is rarely what you want. Force on with `jarvy setup --start-services` if you need it.

---

## Lifecycle integration

| Stage | What Jarvy does |
|-------|-----------------|
| `pre_setup` hook | runs before any tool install |
| Tool install | brew/apt/winget/etc per `[provisioner]` |
| `[env]` apply | writes `.env`, updates shell rc if asked |
| `[services]` auto-start | only if `auto_start = true` and not in CI |
| `post_setup` hook | runs last; good place for `npm install`, `bundle install`, etc. |

Order matters: services start AFTER tools are installed, so Postgres is available to a `post_setup` hook that runs `prisma migrate dev`.

---

## CI and unattended environments

Auto-start is off in CI. To run integration tests against the Compose stack:

```yaml
# GitHub Actions
- name: Provision env
  run: jarvy setup
- name: Start services
  run: jarvy services start --wait-healthy
- name: Run tests
  run: npm run test:integration
- name: Stop services
  if: always()
  run: jarvy services stop
```

`--wait-healthy` (Compose only) blocks until every service reports `healthy`, up to a 120s default timeout. Adjust with `--wait-timeout=300`.

---

## Troubleshooting

- **"port already allocated"** — another process or container is using the port. `jarvy services status` shows the conflict; `docker ps -a` confirms.
- **Services start but app can't connect** — check `docker compose logs <svc>` for a slow boot; add `--wait-healthy` so `jarvy services start` blocks until ready.
- **Compose v1 vs v2** — Jarvy uses `docker compose` (v2). Install via `docker-compose-plugin` on Debian / Ubuntu, or upgrade Docker Desktop on macOS / Windows.
- **Tilt UI didn't open** — `tilt up` is foreground by design. Run `jarvy services start` in a separate terminal, or use `tilt up --stream` for log-only mode.

---

## Next

- [Configuration reference](configuration.md) — full `[services]` schema
- [Cookbook: multi-project Postgres](cookbook/multi-project-postgres.md) — sharing one Postgres across repos
- [Cookbook: github-actions matrix](cookbook/github-actions-matrix.md) — CI patterns
