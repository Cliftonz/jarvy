---
title: "Recipe: Multi-project Postgres on one laptop — Jarvy"
description: "Run different Postgres versions and databases for different projects without port conflicts using Jarvy services and per-project DATABASE_URL."
tags:
  - cookbook
  - services
  - database
---

# Recipe: multi-project Postgres on one laptop

## Problem

You work on two projects. Project A needs Postgres 15 on `:5432` with database `app_dev`. Project B needs Postgres 16 on... also `:5432`, also `app_dev`. They can't both bind the same port. You don't want a Docker daemon constantly running.

---

## Config

Use `[services]` with Docker Compose under the hood, and pin a per-project port + database name:

```toml title="project-a/jarvy.toml"
[provisioner]
docker = "latest"
psql   = "latest"

[services]
enabled    = true
auto_start = true

[env.vars]
POSTGRES_VERSION = "15"
POSTGRES_PORT    = "55432"
POSTGRES_DB      = "app_a_dev"
DATABASE_URL     = "postgres://postgres@localhost:55432/app_a_dev"
```

```yaml title="project-a/docker-compose.yml"
services:
  db:
    image: postgres:${POSTGRES_VERSION}
    ports:
      - "${POSTGRES_PORT}:5432"
    environment:
      POSTGRES_PASSWORD: dev
      POSTGRES_DB: ${POSTGRES_DB}
    volumes:
      - ./.data/postgres:/var/lib/postgresql/data
```

Project B uses the same shape with different ports/versions:

```toml title="project-b/jarvy.toml"
[env.vars]
POSTGRES_VERSION = "16"
POSTGRES_PORT    = "55433"
POSTGRES_DB      = "app_b_dev"
DATABASE_URL     = "postgres://postgres@localhost:55433/app_b_dev"
```

---

## Why it works

| Piece | What it does |
|---|---|
| Pinned per-project port | No collision — Project A on `55432`, B on `55433`. Use any free port. |
| `POSTGRES_VERSION` env | Compose template substitutes the version, so each project pins its server cleanly. |
| `DATABASE_URL` in `[env.vars]` | Your app reads this — works the same in dev, test, CI. |
| `[services] auto_start = true` | `jarvy setup` brings the database up; `jarvy services stop` tears it down. |
| `./.data/postgres` volume | Per-project data. Add `.data/` to `.gitignore`. |

---

## Variations

**Use the system Postgres for one project, Docker for the other:**

```toml title="project-a/jarvy.toml"
[provisioner]
psql = "latest"   # system postgres@15 via brew

[env.vars]
DATABASE_URL = "postgres://$USER@localhost:5432/app_a_dev"
```

System Postgres is faster, takes less RAM, but locks you to whatever version is installed.

**Add Redis next to Postgres:**

```yaml title="docker-compose.yml"
services:
  db:
    image: postgres:16
    ports: ["55432:5432"]
  redis:
    image: redis:7
    ports: ["56379:6379"]
```

```toml title="jarvy.toml"
[env.vars]
DATABASE_URL = "postgres://postgres@localhost:55432/app_dev"
REDIS_URL    = "redis://localhost:56379"
```

**Seed the database after start:**

```toml
[hooks]
post_setup = "until psql $DATABASE_URL -c 'select 1' >/dev/null 2>&1; do sleep 1; done && make db-seed"
```

This waits for Postgres to accept connections, then seeds.

---

## Caveats

- **Port pinning per project is on you.** Pick high ports (50000+) to avoid system services. Document your team's port allocation if it gets crowded.
- **`brew install postgresql` and Docker Postgres can both run simultaneously**, but mixing them on the same project is asking for confusion. Pick one per project.
- **Backups:** the `./.data/postgres` directory is just disk. Snapshot it regularly if the data matters.

---

## See also

- [Services configuration](../configuration.md#services-services)
- [Lifecycle — when services start](../concepts/lifecycle.md)
